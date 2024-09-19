use std::path::PathBuf;

use ahash::HashMap;
use egui::{PointerButton, Resize};
use egui_plot::{Plot, PlotPoint};
use tracing_tape::Span;

use crate::block::Block;

use super::TabViewer;

pub struct TapeTimeline {
    title: String,
    tape_path: PathBuf,
    last_bounds: std::ops::RangeInclusive<f64>,
}

impl TapeTimeline {
    pub fn new<P: Into<PathBuf>>(tape_path: P) -> Self {
        let tape_path = tape_path.into();
        let short_filename = tape_path
            .file_name()
            .map(|f| f.to_string_lossy())
            .unwrap_or_else(|| tape_path.to_string_lossy());

        let title = format!("Timeline {}", short_filename);
        Self {
            title,
            tape_path,
            last_bounds: 0.0..=1.0,
        }
    }

    pub fn id(&self) -> (&PathBuf, &str) {
        (&self.tape_path, "timeline")
    }

    pub fn title(&self) -> egui::WidgetText {
        (&self.title).into()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, viewer: &mut TabViewer) {
        // if let Some(loaded_tape) = state.loaded_tapes.get(&self.tape_path) {
        //     // self.event_points.clear();
        //     // for event in tape.events() {
        //     //     self.event_points.push([event.timestamp as f64, 0.0]);
        //     // }

        //     enum SpanEvent<'a> {
        //         Entered { exit: u64, span: &'a Span<'a> },
        //         Exited,
        //     }

        //     let mut thread_span_events = HashMap::<u64, Vec<(u64, SpanEvent)>>::default();

        //     let global_range = &self.last_bounds;
        //     let time_span = viewer.global_to_time_span(*global_range.start()..*global_range.end());
        //     let timestamp_range = viewer.global_to_tape_timestamp_range(
        //         loaded_tape,
        //         *global_range.start()..*global_range.end(),
        //     );

        //     fn ranges_overlap(a: &std::ops::Range<u64>, b: &std::ops::Range<u64>) -> bool {
        //         a.start < b.end && b.start < a.end
        //     }

        //     let data = loaded_tape.tape.data_for_time_span(time_span);
        //     for data in data.0 {
        //         for span in data.spans() {
        //             for entrance in &span.entrances {
        //                 if !ranges_overlap(&(entrance.enter..entrance.exit), &timestamp_range) {
        //                     continue;
        //                 }

        //                 let thread_events = thread_span_events
        //                     .entry(entrance.thread_id)
        //                     .or_insert_with(Vec::new);
        //                 thread_events.push((
        //                     entrance.enter,
        //                     SpanEvent::Entered {
        //                         exit: entrance.exit,
        //                         span,
        //                     },
        //                 ));
        //                 thread_events.push((entrance.exit, SpanEvent::Exited));
        //             }
        //         }
        //     }

        //     let color = ui.style().visuals.widgets.active.bg_fill;
        //     let mut weak_color = ui.style().visuals.widgets.active.weak_bg_fill;
        //     weak_color[3] = 64;

        //     // let points = Points::new(self.event_points.clone()).shape(MarkerShape::Diamond);
        //     for (thread_name, thread_id) in loaded_tape.tape.threads() {
        //         egui::CollapsingHeader::new(thread_name)
        //             .default_open(true)
        //             .show_unindented(ui, |ui| {
        //                 let width = ui.available_width();
        //                 Resize::default()
        //                     .id_source((thread_id, loaded_tape.tape.path()))
        //                     .resizable([false, true])
        //                     .min_width(width)
        //                     .max_width(width)
        //                     .default_height(200.0)
        //                     .with_stroke(false)
        //                     .show(ui, |ui| {
        //                         Plot::new((thread_id, loaded_tape.tape.path().to_string_lossy()))
        //                             .allow_zoom([true, false])
        //                             .allow_scroll([true, false])
        //                             .x_axis_formatter(viewer.time_axis_formatter())
        //                             .show_grid([true, false])
        //                             // .y_grid_spacer(|_| vec![])
        //                             .show_axes([true, false])
        //                             .link_cursor("global", true, false)
        //                             .link_axis("global", true, false)
        //                             .allow_boxed_zoom(false)
        //                             .auto_bounds(false.into())
        //                             .include_y(0.0)
        //                             .include_y(-5.0)
        //                             .include_x(0.0)
        //                             .include_x(1.0)
        //                             .show_y(false)
        //                             .label_formatter(viewer.label_formatter())
        //                             .show(ui, |plot_ui| {
        //                                 let response = plot_ui.response();
        //                                 if response.drag_started_by(PointerButton::Secondary) {
        //                                     if let Some(hover_pos) = response.hover_pos() {
        //                                         let pos = plot_ui
        //                                             .transform()
        //                                             .value_from_position(hover_pos);
        //                                         *viewer.selected_range = Some(pos.x..pos.x);
        //                                         println!("{:?}", viewer.selected_range);
        //                                     }
        //                                 }
        //                                 if response.dragged_by(PointerButton::Secondary) {
        //                                     match (response.hover_pos(), &mut viewer.selected_range)
        //                                     {
        //                                         (Some(hover_pos), Some(range)) => {
        //                                             let pos = plot_ui
        //                                                 .transform()
        //                                                 .value_from_position(hover_pos);
        //                                             *viewer.selected_range =
        //                                                 Some(range.start..pos.x);
        //                                         }
        //                                         _ => {}
        //                                     }
        //                                 }
        //                                 if response.clicked_by(PointerButton::Secondary) {
        //                                     *viewer.selected_range = None;
        //                                 }
        //                                 let mut bounds = plot_ui.plot_bounds();
        //                                 *viewer.timeline_center = viewer.global_time_span.start
        //                                     + time::Duration::seconds_f64(
        //                                         bounds.min()[0] + bounds.width() / 2.0,
        //                                     );

        //                                 let y_range = bounds.range_y();
        //                                 if *y_range.end() > 0.0 {
        //                                     bounds.translate_y(-*y_range.end());
        //                                     plot_ui.set_plot_bounds(bounds);
        //                                 }

        //                                 let global_range = plot_ui.plot_bounds().range_x();
        //                                 self.last_bounds = global_range.clone();

        //                                 if let Some(thread_events) =
        //                                     thread_span_events.get_mut(&thread_id)
        //                                 {
        //                                     thread_events.sort_by_key(|(timestamp, _)| *timestamp);
        //                                     let mut level = 0;
        //                                     for (timestamp, event) in thread_events {
        //                                         match event {
        //                                             SpanEvent::Entered { exit, span } => {
        //                                                 let enter = viewer.tape_to_global(
        //                                                     loaded_tape,
        //                                                     *timestamp,
        //                                                 );
        //                                                 let exit = viewer
        //                                                     .tape_to_global(loaded_tape, *exit);
        //                                                 let callsite = loaded_tape
        //                                                     .tape
        //                                                     .callsite(span.callsite);

        //                                                 let block = Block::new(
        //                                                     enter..exit,
        //                                                     callsite
        //                                                         .map(|c| c.name.to_string())
        //                                                         .unwrap_or_default(),
        //                                                     level,
        //                                                     color,
        //                                                 );
        //                                                 plot_ui.add(block);
        //                                                 level += 1;
        //                                             }
        //                                             SpanEvent::Exited => {
        //                                                 level -= 1;
        //                                             }
        //                                         }
        //                                     }
        //                                 }

        //                                 if let Some(selected_range) = &viewer.selected_range {
        //                                     let bounds = plot_ui.plot_bounds();
        //                                     let l = selected_range.start;
        //                                     let r = selected_range.end;
        //                                     let t = bounds.max()[1];
        //                                     let b = bounds.min()[1];
        //                                     plot_ui.add(
        //                                         egui_plot::Polygon::new(vec![
        //                                             [l, t],
        //                                             [r, t],
        //                                             [r, b],
        //                                             [l, b],
        //                                         ])
        //                                         .fill_color(weak_color),
        //                                     );

        //                                     let time_span =
        //                                         viewer.global_to_time_span(selected_range.clone());
        //                                     let duration = time_span.end - time_span.start;

        //                                     plot_ui.add(
        //                                         egui_plot::Text::new(
        //                                             PlotPoint::new((l + r) / 2.0, (t + b) / 2.0),
        //                                             format!("{}", duration),
        //                                         )
        //                                         .color(egui::Color32::WHITE),
        //                                     );
        //                                 }
        //                             });
        //                     });
        //             });
        //     }
        // } else {
        //     ui.label("Loading tape...");
        // }
    }
}
