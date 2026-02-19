//! Trace Deck is a tool for visualizing and analyzing trace data recorded by
//! the [tracing-tape-recorder](https://crates.io/crates/tracing-tape-recorder) crate.
use std::{path::PathBuf, sync::Arc};

use clap::Parser;
use eframe::egui;
use egui_dock::{DockArea, DockState, Style};
use state::{LoadedTape, LoadedTapes, State};
use tabs::{Tab, TabViewer};
use tracing_tape_parser::Tape;

mod state;
pub(crate) mod statistics;
mod tabs;
pub(crate) mod timeline;
pub(crate) mod utils;

#[derive(Debug, Default, Parser)]
struct Args {
    tape_files: Vec<String>,

    #[clap(short, long)]
    num_threads: Option<usize>,
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), eframe::Error> {
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = egui::ViewportBuilder::default().with_maximized(true);

    eframe::run_native(
        "Trace Deck",
        native_options,
        Box::new(|cc| Ok(Box::new(TraceDeck::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast;

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let window = web_sys::window().expect("no global `window` exists");
        let document = window.document().expect("should have a document on window");
        let canvas = document
            .get_element_by_id("eframe")
            .expect("should have a canvas element with id `eframe-canvas`")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("element with id `eframe` should be a canvas element");

        let _ = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(TraceDeck::new(cc)))),
            )
            .await;
    });
}

struct TraceDeck {
    dock_state: DockState<Tab>,
    state: State,
}

impl TraceDeck {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that
        // you can use for e.g. egui::PaintCallback.

        let args = Args::parse();

        if let Some(num_threads) = args.num_threads {
            rayon::ThreadPoolBuilder::new()
                .num_threads(num_threads)
                .build_global()
                .unwrap();
        }

        let (dock_state, tapes) = if args.tape_files.is_empty() {
            (DockState::new(vec![Tab::welcome()]), LoadedTapes::default())
        } else {
            Self::load_files(args.tape_files.iter().map(|path| {
                let path = path.into();
                let file = std::fs::read(&path).unwrap();
                (path, file.into())
            }))
            .unwrap()
        };

        Self {
            dock_state,
            state: tapes.into(),
        }
    }

    fn load_files<I>(files: I) -> std::io::Result<(DockState<Tab>, LoadedTapes)>
    where
        I: Iterator<Item = (PathBuf, Arc<[u8]>)>,
    {
        let mut dock_state = DockState::new(vec![Tab::global_timeline()]);
        let mut tapes: Vec<LoadedTape> = Vec::with_capacity(files.size_hint().1.unwrap_or(0));

        for (path, file) in files {
            let path = path.into();

            // De-duplicate files.
            // Linear search is fine, we should only ever have a few files.
            if tapes.iter().find(|t| &t.path == &path).is_some() {
                continue;
            }

            let tape = Tape::parse(&file);

            tapes.push(LoadedTape {
                path,
                tape,
                time_offset: time::Duration::ZERO,
            });
        }

        let mut paths = tapes.iter().map(|t| &t.path);
        if let Some(path) = paths.next() {
            let main_surface = dock_state.main_surface_mut();
            let root_index = egui_dock::NodeIndex::root();

            let [_, first_timeline] =
                main_surface.split_above(root_index, 0.9, vec![Tab::timeline(path)]);

            let [timeline_node, callsites] =
                main_surface.split_left(first_timeline, 0.1, vec![Tab::callsites()]);

            let [_callsites, _details] =
                main_surface.split_below(callsites, 0.5, vec![Tab::details()]);

            let [timeline_node, event_node] =
                main_surface.split_right(timeline_node, 0.5, vec![Tab::events(path)]);

            for (index, path) in paths.enumerate() {
                let fraction = (index as f32 + 1.0) / (index as f32 + 2.0);
                main_surface.split_below(timeline_node, fraction, vec![Tab::timeline(path)]);
                main_surface.split_right(event_node, fraction, vec![Tab::events(path)]);
            }
        }

        Ok((dock_state, tapes.into()))
    }
}

impl eframe::App for TraceDeck {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut global_time_span: Option<std::ops::Range<time::OffsetDateTime>> = None;
            for tape in &*self.state.loaded_tapes {
                let tape_time_span = tape.adjusted_timespan();
                if let Some(acc) = global_time_span {
                    let min = acc.start.min(tape_time_span.start);
                    let max = acc.end.max(tape_time_span.end);
                    global_time_span = Some(min..max);
                } else {
                    global_time_span = Some(tape_time_span);
                }
            }
            let global_time_span = global_time_span.unwrap_or_else(|| {
                let now = time::OffsetDateTime::from_unix_timestamp(0).expect("time");
                now..now + time::Duration::MINUTE
            });

            let mut viewer = TabViewer {
                // tapes: &self.tapes,
                state: &mut self.state,
                global_time_span,
                new_tabs: vec![],
            };

            DockArea::new(&mut self.dock_state)
                .style(Style::from_egui(ui.style().as_ref()))
                .show_inside(ui, &mut viewer);

            if !viewer.new_tabs.is_empty() {
                self.dock_state.add_window(viewer.new_tabs);
            }
        });
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if !raw_input.dropped_files.is_empty() {
            let (dock_state, tapes) = Self::load_files(raw_input.dropped_files.iter().map(|f| {
                let path = f.path.clone().unwrap_or_else(|| (&f.name).into());
                let bytes = f
                    .bytes
                    .clone()
                    .unwrap_or_else(|| std::fs::read(&path).unwrap().into());
                (path, bytes)
            }))
            .unwrap();
            self.dock_state = dock_state;
            self.state = tapes.into();
        }
    }
}
