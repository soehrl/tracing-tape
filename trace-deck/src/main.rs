use std::{ops::{Deref, DerefMut}, path::{Path, PathBuf}};

use clap::Parser;
use eframe::egui;
use egui_dock::{DockArea, DockState, Style};
use tabs::{Tab, TabViewer};
use trace_deck::Tape;

pub mod block;
mod tabs;

#[derive(Debug, Default, Parser)]
struct Args {
    tape_files: Vec<String>,

    #[clap(short, long)]
    num_threads: Option<usize>,
}

fn main() -> Result<(), eframe::Error> {
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = egui::ViewportBuilder::default().with_maximized(true);

    eframe::run_native(
        "Trace Deck",
        native_options,
        Box::new(|cc| Ok(Box::new(TraceDeck::new(cc)))),
    )
}

#[derive(Debug)]
pub struct LoadedTape {
    pub path: PathBuf,
    pub tape: Tape,
    pub time_offset: time::Duration,
}

impl LoadedTape {
    pub fn adjusted_timespan(&self) -> std::ops::Range<time::OffsetDateTime> {
        let tape_time_span = self.tape.time_span();
        tape_time_span.start + self.time_offset..tape_time_span.end + self.time_offset
    }
}

#[derive(Debug, Default)]
pub struct LoadedTapes(Vec<LoadedTape>);

impl Deref for LoadedTapes {
    type Target = Vec<LoadedTape>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LoadedTapes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl LoadedTapes {
    pub fn get(&self, path: &Path) -> Option<&LoadedTape> {
        self.0.iter().find(|tape| tape.path == path)
    }

    pub fn get_mut(&mut self, path: &Path) -> Option<&mut LoadedTape> {
        self.0.iter_mut().find(|tape| tape.path == path)
    }
}

struct TraceDeck {
    dock_state: DockState<Tab>,
    tapes: LoadedTapes,
    selected_range: Option<std::ops::Range<f64>>,
    utc_offset: time::UtcOffset,
    global_center: time::OffsetDateTime,
}

impl TraceDeck {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.

        let args = Args::parse();

        if let Some(num_threads) = args.num_threads {
            rayon::ThreadPoolBuilder::new()
                .num_threads(num_threads)
                .build_global()
                .unwrap();
        }

        let (dock_state, tapes) = if args.tape_files.is_empty() {
            todo!()
        } else {
            Self::load_files(args.tape_files.iter()).unwrap()
        };

        let utc_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);

        Self {
            dock_state,
            tapes,
            utc_offset,
            selected_range: None,
            global_center: time::OffsetDateTime::now_utc(),
        }
    }

    fn load_files(files: impl Iterator<Item = impl Into<PathBuf>>) -> std::io::Result<(DockState<Tab>, LoadedTapes)> {
        let mut dock_state = DockState::new(vec![Tab::global_timeline()]);
        let mut tapes: Vec<LoadedTape> = Vec::with_capacity(files.size_hint().1.unwrap_or(0));

        for path in files {
            let path = path.into();

            // De-duplicate files.
            // Linear search is fine, we should only ever have a few files.
            if tapes.iter().find(|t| &t.path == &path).is_some() {
                continue;
            }

            let tape = Tape::from_path(&path).unwrap();

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

            let [timeline_node, event_node] =
                main_surface.split_right(first_timeline, 0.5, vec![Tab::events(path)]);

            for (index, path) in paths.enumerate() {
                let fraction = (index as f32 + 1.0) / (index as f32 + 2.0);
                main_surface.split_below(timeline_node, fraction, vec![Tab::timeline(path)]);
                main_surface.split_right(event_node, fraction, vec![Tab::events(path)]);
            }
        }

        Ok((dock_state, LoadedTapes(tapes)))
    }
}

impl eframe::App for TraceDeck {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut global_time_span: Option<std::ops::Range<time::OffsetDateTime>> = None;
            for tape in &*self.tapes {
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
                let now = time::OffsetDateTime::now_utc();
                now..now + time::Duration::MINUTE
            });

            let mut viewer = TabViewer {
                tapes: &self.tapes,
                utc_offset: self.utc_offset,
                global_time_span,
                selected_range: &mut self.selected_range,
                timeline_center: &mut self.global_center,
            };

            DockArea::new(&mut self.dock_state)
                .style(Style::from_egui(ui.style().as_ref()))
                .show_inside(ui, &mut viewer);
        });
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if !raw_input.dropped_files.is_empty() {
            let (dock_state, tapes) = Self::load_files(raw_input.dropped_files.iter().map(|f| f.path.as_ref().unwrap())).unwrap();
            self.dock_state = dock_state;
            self.tapes = tapes;
        }
    }
}
