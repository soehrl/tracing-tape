use std::{ops::RangeInclusive, path::PathBuf};

use ahash::{HashMap, HashSet};
use clap::Parser;
use eframe::egui;
use egui::{Align, PointerButton, Resize};
use egui_dock::{DockArea, DockState, Style};
use egui_extras::{Column, TableBuilder};
use egui_plot::{GridInput, GridMark, Plot, PlotPoint};
use trace_deck::Tape;
use tracing_tape::Span;

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
        "Tape Deck",
        native_options,
        Box::new(|cc| Ok(Box::new(TapeDeck::new(cc)))),
    )
}

struct TabViewer<'a> {
    tapes: &'a HashMap<PathBuf, LoadedTape>,
    utc_offset: time::UtcOffset,
    global_time_span: std::ops::Range<time::OffsetDateTime>,
    selected_range: &'a mut Option<std::ops::Range<f64>>,
    timeline_center: &'a mut time::OffsetDateTime,
}

impl TabViewer<'_> {
    fn global_max_seconds(&self) -> f64 {
        (self.global_time_span.end - self.global_time_span.start).as_seconds_f64()
    }

    fn tape_to_global(&self, tape: &LoadedTape, timestamp: u64) -> f64 {
        let tape_time = tape.tape.time_span().start + time::Duration::nanoseconds(timestamp as i64);
        let global_offset = tape_time + tape.time_offset - self.global_time_span.start;
        global_offset.as_seconds_f64()
    }

    fn global_to_tape_timestamp(&self, tape: &LoadedTape, global: f64) -> u64 {
        let global_time = self.global_time_span.start + time::Duration::seconds_f64(global);
        let tape_time = global_time - tape.time_offset;
        if tape_time < tape.tape.time_span().start {
            0
        } else {
            (tape_time - tape.tape.time_span().start).whole_nanoseconds() as u64
        }
    }

    fn global_to_tape_timestamp_range(
        &self,
        tape: &LoadedTape,
        global: std::ops::Range<f64>,
    ) -> std::ops::Range<u64> {
        let start = self.global_to_tape_timestamp(tape, global.start);
        let end = self.global_to_tape_timestamp(tape, global.end);
        start..end
    }

    fn tape_to_global_span(
        &self,
        tape: &LoadedTape,
        span: std::ops::Range<u64>,
    ) -> std::ops::Range<f64> {
        let start = self.tape_to_global(tape, span.start);
        let end = self.tape_to_global(tape, span.end);
        start..end
    }

    fn global_to_tape(&self, tape: &LoadedTape, global: f64) -> time::OffsetDateTime {
        let global_time = self.global_time_span.start + time::Duration::seconds_f64(global);
        global_time - tape.time_offset
    }

    // fn global_to_tape_span(
    //     &self,
    //     tape: &LoadedTape,
    //     span: std::ops::Range<f64>,
    // ) -> std::ops::Range<u64> {
    //     let start = self.global_to_tape(tape, span.start);
    //     let end = self.global_to_tape(tape, span.end);

    //     let start = start - tape.tape.time_span().start;
    // }

    fn global_to_time(&self, global: f64) -> time::OffsetDateTime {
        self.global_time_span.start + time::Duration::seconds_f64(global)
    }

    fn global_to_time_span(
        &self,
        span: std::ops::Range<f64>,
    ) -> std::ops::Range<time::OffsetDateTime> {
        let start = self.global_to_time(span.start);
        let end = self.global_to_time(span.end);
        start..end
    }

    fn time_to_global(&self, time: time::OffsetDateTime) -> f64 {
        (time - self.global_time_span.start).as_seconds_f64()
    }

    fn time_to_global_span(
        &self,
        span: std::ops::Range<time::OffsetDateTime>,
    ) -> std::ops::Range<f64> {
        let start = self.time_to_global(span.start);
        let end = self.time_to_global(span.end);
        start..end
    }

    fn time_axis_formatter(&self) -> impl Fn(GridMark, &RangeInclusive<f64>) -> String {
        let base = self.global_time_span.start;
        move |grid_mark, _range| {
            let time = base + time::Duration::seconds_f64(grid_mark.value);
            let format = time::macros::format_description!("[hour]:[minute]:[second]");

            time.format(&format).unwrap_or_else(|_| time.to_string())
        }
    }

    fn time_grid_spacer(&self) -> impl Fn(GridInput) -> Vec<GridMark> {
        move |input| {
            let (min, max) = input.bounds;
            let range = max - min;
            let duration = time::Duration::seconds_f64(range);

            if duration <= time::Duration::SECOND {}

            vec![]
        }
    }

    fn label_formatter(&self) -> impl Fn(&str, &PlotPoint) -> String {
        let format = time::macros::format_description!("[hour]:[minute]:[second]");
        let global_time_base = self.global_time_span.start;

        move |str, point| {
            let time = global_time_base + time::Duration::seconds_f64(point.x);
            let time_str = time
                .format(&format)
                .unwrap_or_else(|_| point.x.floor().to_string());
            return format!("{} {}.{}", str, time_str, time.time().nanosecond());
        }
    }
}

impl egui_dock::TabViewer for TabViewer<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        match tab {
            Tab::GlobalTimeline(timeline) => egui::Id::new(timeline.id()),
            Tab::Events(tape) => egui::Id::new(tape.id()),
            Tab::Timeline(tape) => egui::Id::new(tape.id()),
        }
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            Tab::GlobalTimeline(timeline) => timeline.title(),
            Tab::Events(tape) => tape.title(),
            Tab::Timeline(tape) => tape.title(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            Tab::GlobalTimeline(timeline) => timeline.ui(ui, self),
            Tab::Events(tape) => tape.ui(ui, self),
            Tab::Timeline(tape) => tape.ui(ui, self),
        }
    }
}

struct Block {
    span: std::ops::Range<f64>,
    name: String,
    level: usize,
    bg: egui::Color32,
}

impl Block {
    fn new(span: std::ops::Range<f64>, name: String, level: usize, color: egui::Color32) -> Self {
        Self {
            span,
            name,
            level,
            bg: color,
        }
    }

    fn min(&self) -> [f64; 2] {
        [self.span.start, -(self.level as f64 + 0.9)]
    }

    fn max(&self) -> [f64; 2] {
        [self.span.end, -(self.level as f64 + 0.1)]
    }
}

impl egui_plot::PlotItem for Block {
    fn shapes(
        &self,
        ui: &egui::Ui,
        transform: &egui_plot::PlotTransform,
        shapes: &mut Vec<egui::Shape>,
    ) {
        // ui.style().visuals.faint_bg_color
        let bounds_x = transform.bounds().range_x();
        let mut visible_x = [self.min()[0], self.max()[0]];
        if visible_x[0] < *bounds_x.start() {
            visible_x[0] = *bounds_x.start();
        }
        if visible_x[1] > *bounds_x.end() {
            visible_x[1] = *bounds_x.end();
        }

        if visible_x[0] >= visible_x[1] {
            return;
        }

        let min_x = transform.position_from_point_x(*bounds_x.start());
        let max_x = transform.position_from_point_x(*bounds_x.end());

        let rect =
            transform.rect_from_values(&PlotPoint::from(self.min()), &PlotPoint::from(self.max()));

        shapes.push(egui::Shape::Rect(egui::epaint::RectShape {
            rect,
            rounding: egui::Rounding::same(2.0),
            fill: self.color(),
            stroke: Default::default(),
            blur_width: 0.0,
            fill_texture_id: Default::default(),
            uv: egui::epaint::Rect::ZERO,
        }));

        ui.fonts(|fonts| {
            let text = self.name.clone();
            let font_id = egui::FontId::default();
            let color = egui::Color32::WHITE;

            let galley = fonts.layout_no_wrap(text, font_id, color);
            if galley.rect.width() > rect.width() {
                return;
            }

            let anchor = egui::Align2::CENTER_CENTER;
            let pos = rect.center();
            let mut text_rect = anchor.anchor_size(pos, galley.size());
            if text_rect.min.x < min_x {
                let offset = f32::min(
                    min_x - text_rect.min.x,
                    (rect.width() - text_rect.width()) / 2.0,
                );
                text_rect = text_rect.translate(egui::vec2(offset, 0.0));
            }
            if text_rect.max.x > max_x {
                let offset = f32::min(
                    text_rect.max.x - max_x,
                    (rect.width() - text_rect.width()) / 2.0,
                );
                text_rect = text_rect.translate(egui::vec2(-offset, 0.0));
            }
            shapes.push(egui::Shape::galley(text_rect.min, galley, color));
        });
    }

    fn initialize(&mut self, x_range: std::ops::RangeInclusive<f64>) {
        // todo!()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn color(&self) -> egui::Color32 {
        self.bg
        // todo!()
    }

    fn highlight(&mut self) {
        // todo!()
    }

    fn highlighted(&self) -> bool {
        false
    }

    fn allow_hover(&self) -> bool {
        true
    }

    fn geometry(&self) -> egui_plot::PlotGeometry<'_> {
        egui_plot::PlotGeometry::Rects
    }

    fn bounds(&self) -> egui_plot::PlotBounds {
        egui_plot::PlotBounds::from_min_max(self.min(), self.max())
    }

    fn id(&self) -> Option<egui::Id> {
        None
    }

    fn find_closest(
        &self,
        point: egui::Pos2,
        transform: &egui_plot::PlotTransform,
    ) -> Option<egui_plot::ClosestElem> {
        let rect =
            transform.rect_from_values(&PlotPoint::from(self.min()), &PlotPoint::from(self.max()));
        let dist_sq = rect.distance_sq_to_pos(point);
        Some(egui_plot::ClosestElem { index: 0, dist_sq })
    }

    fn on_hover(
        &self,
        elem: egui_plot::ClosestElem,
        shapes: &mut Vec<egui::Shape>,
        cursors: &mut Vec<egui_plot::Cursor>,
        plot: &egui_plot::PlotConfig<'_>,
        label_formatter: &egui_plot::LabelFormatter<'_>,
    ) {
        // println!("hovering over block");
        cursors.push(egui_plot::Cursor::Vertical { x: self.min()[0] });
    }
}

#[derive(Default)]
struct GlobalTimeline {}

impl GlobalTimeline {
    fn id(&self) -> &str {
        "timeline"
    }

    fn title(&self) -> egui::WidgetText {
        "Global Timeline".into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, viewer: &mut TabViewer) {
        let color = ui.style().visuals.widgets.active.bg_fill;
        Plot::new("Global Timeline")
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .allow_double_click_reset(false)
            .allow_boxed_zoom(false)
            .include_x(0.0)
            .include_x(viewer.global_max_seconds())
            .include_y(0.0)
            .include_y(-(viewer.tapes.len() as f64))
            // .label_formatter(|x,_| format!("{:.3}s", x))
            .x_axis_formatter(viewer.time_axis_formatter())
            // .x_grid_spacer(viewer.time_grid_spacer())
            .y_grid_spacer(|_| vec![])
            .link_cursor("global", true, false)
            .show(ui, |plot_ui| {
                for (level, (path, tape)) in viewer.tapes.iter().enumerate() {
                    let b = Block::new(
                        viewer.time_to_global_span(tape.adjusted_timespan()),
                        path.to_string_lossy().to_string(),
                        level,
                        color,
                    );
                    plot_ui.add(b);
                }
                // viewer.tapes.values().for_each(|tape| {
                //     let tape_time_span = tape.adjusted_timespan();
                //     let points = Points::new(vec![
                //         [tape_time_span.start.as_seconds_f64(), 0.0],
                //         [tape_time_span.end.as_seconds_f64(), 0.0],
                //     ])
                //     .shape(MarkerShape::Diamond);

                //     plot_ui.points(points);
                // });
                // // plot_ui.points(points);
            });
    }
}

struct TapeTimeline {
    title: String,
    tape_path: PathBuf,
    last_bounds: std::ops::RangeInclusive<f64>,
}

impl TapeTimeline {
    fn new<P: Into<PathBuf>>(tape_path: P) -> Self {
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

    fn id(&self) -> (&PathBuf, &str) {
        (&self.tape_path, "timeline")
    }

    fn title(&self) -> egui::WidgetText {
        (&self.title).into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, viewer: &mut TabViewer) {
        if let Some(loaded_tape) = viewer.tapes.get(&self.tape_path) {
            // self.event_points.clear();
            // for event in tape.events() {
            //     self.event_points.push([event.timestamp as f64, 0.0]);
            // }

            enum SpanEvent<'a> {
                Entered { exit: u64, span: &'a Span<'a> },
                Exited,
            }

            let mut thread_span_events = HashMap::<u64, Vec<(u64, SpanEvent)>>::default();

            let global_range = &self.last_bounds;
            let time_span = viewer.global_to_time_span(*global_range.start()..*global_range.end());
            let timestamp_range = viewer.global_to_tape_timestamp_range(
                loaded_tape,
                *global_range.start()..*global_range.end(),
            );

            fn ranges_overlap(a: &std::ops::Range<u64>, b: &std::ops::Range<u64>) -> bool {
                a.start < b.end && b.start < a.end
            }

            let data = loaded_tape.tape.data_for_time_span(time_span);
            for data in data.0 {
                for span in data.spans() {
                    for entrance in &span.entrances {
                        if !ranges_overlap(&(entrance.enter..entrance.exit), &timestamp_range) {
                            continue;
                        }

                        let thread_events = thread_span_events
                            .entry(entrance.thread_id)
                            .or_insert_with(Vec::new);
                        thread_events.push((
                            entrance.enter,
                            SpanEvent::Entered {
                                exit: entrance.exit,
                                span,
                            },
                        ));
                        thread_events.push((entrance.exit, SpanEvent::Exited));
                    }
                }
            }

            let color = ui.style().visuals.widgets.active.bg_fill;
            let mut weak_color = ui.style().visuals.widgets.active.weak_bg_fill;
            weak_color[3] = 64;

            // let points = Points::new(self.event_points.clone()).shape(MarkerShape::Diamond);
            for (thread_name, thread_id) in loaded_tape.tape.threads() {
                egui::CollapsingHeader::new(thread_name)
                    .default_open(true)
                    .show_unindented(ui, |ui| {
                        let width = ui.available_width();
                        Resize::default()
                            .id_source((thread_id, loaded_tape.tape.path()))
                            .resizable([false, true])
                            .min_width(width)
                            .max_width(width)
                            .default_height(200.0)
                            .with_stroke(false)
                            .show(ui, |ui| {
                                Plot::new((thread_id, loaded_tape.tape.path().to_string_lossy()))
                                    .allow_zoom([true, false])
                                    .allow_scroll([true, false])
                                    .x_axis_formatter(viewer.time_axis_formatter())
                                    .show_grid([true, false])
                                    // .y_grid_spacer(|_| vec![])
                                    .show_axes([true, false])
                                    .link_cursor("global", true, false)
                                    .link_axis("global", true, false)
                                    .allow_boxed_zoom(false)
                                    .auto_bounds(false.into())
                                    .include_y(0.0)
                                    .include_y(-5.0)
                                    .include_x(0.0)
                                    .include_x(1.0)
                                    .show_y(false)
                                    .label_formatter(viewer.label_formatter())
                                    .show(ui, |plot_ui| {
                                        let response = plot_ui.response();
                                        if response.drag_started_by(PointerButton::Secondary) {
                                            if let Some(hover_pos) = response.hover_pos() {
                                                let pos = plot_ui
                                                    .transform()
                                                    .value_from_position(hover_pos);
                                                *viewer.selected_range = Some(pos.x..pos.x);
                                                println!("{:?}", viewer.selected_range);
                                            }
                                        }
                                        if response.dragged_by(PointerButton::Secondary) {
                                            match (response.hover_pos(), &mut viewer.selected_range)
                                            {
                                                (Some(hover_pos), Some(range)) => {
                                                    let pos = plot_ui
                                                        .transform()
                                                        .value_from_position(hover_pos);
                                                    *viewer.selected_range =
                                                        Some(range.start..pos.x);
                                                }
                                                _ => {}
                                            }
                                        }
                                        if response.clicked_by(PointerButton::Secondary) {
                                            *viewer.selected_range = None;
                                        }
                                        let mut bounds = plot_ui.plot_bounds();
                                        *viewer.timeline_center = viewer.global_time_span.start
                                            + time::Duration::seconds_f64(
                                                bounds.min()[0] + bounds.width() / 2.0,
                                            );

                                        let y_range = bounds.range_y();
                                        if *y_range.end() > 0.0 {
                                            bounds.translate_y(-*y_range.end());
                                            plot_ui.set_plot_bounds(bounds);
                                        }

                                        let global_range = plot_ui.plot_bounds().range_x();
                                        self.last_bounds = global_range.clone();

                                        if let Some(thread_events) =
                                            thread_span_events.get_mut(&thread_id)
                                        {
                                            thread_events.sort_by_key(|(timestamp, _)| *timestamp);
                                            let mut level = 0;
                                            for (timestamp, event) in thread_events {
                                                match event {
                                                    SpanEvent::Entered { exit, span } => {
                                                        let enter = viewer.tape_to_global(
                                                            loaded_tape,
                                                            *timestamp,
                                                        );
                                                        let exit = viewer
                                                            .tape_to_global(loaded_tape, *exit);
                                                        let callsite = loaded_tape
                                                            .tape
                                                            .callsite(span.callsite);

                                                        let block = Block::new(
                                                            enter..exit,
                                                            callsite
                                                                .map(|c| c.name.to_string())
                                                                .unwrap_or_default(),
                                                            level,
                                                            color,
                                                        );
                                                        plot_ui.add(block);
                                                        level += 1;
                                                    }
                                                    SpanEvent::Exited => {
                                                        level -= 1;
                                                    }
                                                }
                                            }
                                        }

                                        if let Some(selected_range) = &viewer.selected_range {
                                            let bounds = plot_ui.plot_bounds();
                                            let l = selected_range.start;
                                            let r = selected_range.end;
                                            let t = bounds.max()[1];
                                            let b = bounds.min()[1];
                                            plot_ui.add(
                                                egui_plot::Polygon::new(vec![
                                                    [l, t],
                                                    [r, t],
                                                    [r, b],
                                                    [l, b],
                                                ])
                                                .fill_color(weak_color),
                                            );

                                            let time_span =
                                                viewer.global_to_time_span(selected_range.clone());
                                            let duration = time_span.end - time_span.start;

                                            plot_ui.add(
                                                egui_plot::Text::new(
                                                    PlotPoint::new((l + r) / 2.0, (t + b) / 2.0),
                                                    format!("{}", duration),
                                                )
                                                .color(egui::Color32::WHITE),
                                            );
                                        }
                                    });
                            });
                    });
            }
        } else {
            ui.label("Loading tape...");
        }
    }
}

struct TapeEvents {
    title: String,
    tape_path: PathBuf,
}

impl TapeEvents {
    fn new<P: Into<PathBuf>>(tape_path: P) -> Self {
        let tape_path = tape_path.into();
        let short_filename = tape_path
            .file_name()
            .map(|f| f.to_string_lossy())
            .unwrap_or_else(|| tape_path.to_string_lossy());

        let title = format!("Timeline {}", short_filename);
        Self { title, tape_path }
    }

    fn id(&self) -> (&PathBuf, &str) {
        (&self.tape_path, "events")
    }

    fn title(&self) -> egui::WidgetText {
        (&self.title).into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, viewer: &mut TabViewer) {
        let LoadedTape { tape, time_offset } = if let Some(tape) = viewer.tapes.get(&self.tape_path)
        {
            tape
        } else {
            return;
        };

        let available_height = ui.available_height();

        let offset = tape.find_event_offset(*viewer.timeline_center - *time_offset);

        TableBuilder::new(ui)
            .auto_shrink(false)
            .max_scroll_height(available_height)
            .column(Column::auto())
            .column(Column::remainder().at_least(100.0))
            .cell_layout(egui::Layout::left_to_right(Align::LEFT))
            .scroll_to_row(offset as usize, Some(egui::Align::Center))
            .header(18.0, |mut header| {
                header.col(|ui| {
                    ui.label("Level");
                });
                header.col(|ui| {
                    ui.label("Second column");
                });
            })
            .body(|body| {
                let row_height = 18.0;

                let mut events = tape.events();
                let event_count = events.remaining_len();
                let mut last_index = None;

                body.rows(row_height, event_count, move |mut row| {
                    let row_index = row.index();

                    if let Some(i) = last_index {
                        debug_assert_eq!(row_index, i + 1);
                        last_index = Some(row_index);
                    } else {
                        last_index = Some(row_index);
                        events.skip_n(row_index);
                    }

                    let event = events.next().expect("event index out of bounds");

                    if let Some(event) = event {
                        if let Some(callsite) = tape.callsite(event.callsite) {
                            row.col(|ui| {
                                ui.label(callsite.level.as_str());
                            });
                            row.col(|ui| {
                                callsite.field_names.iter().zip(&event.values).for_each(
                                    |(name, value)| {
                                        if name == "message" {
                                            ui.label(format!("{value}"));
                                        } else {
                                            let _ = ui.selectable_label(
                                                false,
                                                format!("{name}: {value}"),
                                            );
                                        }
                                    },
                                );
                            });
                        }
                    }
                });
            });
    }
}

enum Tab {
    GlobalTimeline(GlobalTimeline),
    Events(TapeEvents),
    Timeline(TapeTimeline),
}

impl Tab {
    fn global_timeline() -> Self {
        Self::GlobalTimeline(GlobalTimeline {})
    }

    fn events<P: Into<PathBuf>>(tape_path: P) -> Self {
        Self::Events(TapeEvents::new(tape_path))
    }

    fn timeline<P: Into<PathBuf>>(tape_path: P) -> Self {
        Self::Timeline(TapeTimeline::new(tape_path))
    }
}

#[repr(transparent)]
struct AdjustedDateTime(time::OffsetDateTime);

struct LoadedTape {
    tape: Tape,
    time_offset: time::Duration,
}

impl LoadedTape {
    fn adjust_date_time(&self, t: time::OffsetDateTime) -> AdjustedDateTime {
        AdjustedDateTime(t + self.time_offset)
    }

    fn adjusted_timespan(&self) -> std::ops::Range<time::OffsetDateTime> {
        let tape_time_span = self.tape.time_span();
        tape_time_span.start + self.time_offset..tape_time_span.end + self.time_offset
    }
}

struct TapeDeck {
    dock_state: DockState<Tab>,
    tapes: HashMap<PathBuf, LoadedTape>,
    selected_range: Option<std::ops::Range<f64>>,
    utc_offset: time::UtcOffset,
    global_center: time::OffsetDateTime,
}

impl TapeDeck {
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

        let mut tapes = HashMap::default();

        let mut dock_state = DockState::new(vec![Tab::global_timeline()]);

        for path in &args.tape_files {
            tapes
                .entry(PathBuf::from(path))
                .or_insert_with(|| LoadedTape {
                    tape: Tape::from_path(path).unwrap(),
                    time_offset: time::Duration::ZERO,
                });
        }

        // We want the paths in the original order, but de-duplicated.
        let mut inserted = HashSet::default();
        let mut paths_dedup = args.tape_files.iter().filter(|path| inserted.insert(*path));

        if let Some(path) = paths_dedup.next() {
            let main_surface = dock_state.main_surface_mut();
            let root_index = egui_dock::NodeIndex::root();

            let [_, first_timeline] =
                main_surface.split_above(root_index, 0.9, vec![Tab::timeline(path)]);

            let [timeline_node, event_node] =
                main_surface.split_right(first_timeline, 0.5, vec![Tab::events(path)]);

            for (index, path) in paths_dedup.enumerate() {
                let fraction = (index as f32 + 1.0) / (index as f32 + 2.0);
                main_surface.split_below(timeline_node, fraction, vec![Tab::timeline(path)]);
                main_surface.split_right(event_node, fraction, vec![Tab::events(path)]);
            }
        }

        let utc_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
        // let time_span = tapes
        //     .values()
        //     .fold(None, |acc: Option<std::ops::Range::OffsetDateTime>, tape| {
        //         if let Some(acc) = acc {
        //             Some(acc.min(tape.timestamp_base()))
        //         } else {
        //             Some(tape.timestamp_base())
        //         }
        //     });
        // let time_base = time_span.unwrap_or_else(|| time::OffsetDateTime::now_utc());

        Self {
            dock_state,
            tapes,
            utc_offset,
            selected_range: None,
            global_center: time::OffsetDateTime::now_utc(),
        }
    }
}

impl eframe::App for TapeDeck {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut global_time_span: Option<std::ops::Range<time::OffsetDateTime>> = None;
            for tape in self.tapes.values() {
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
}
