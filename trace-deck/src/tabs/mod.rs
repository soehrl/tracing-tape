use std::{ops::RangeInclusive, path::PathBuf};

use ahash::HashMap;
use egui_plot::{GridInput, GridMark, PlotPoint};
use trace_deck::Tape;

mod welcome;

mod global_timeline;
use global_timeline::GlobalTimeline;

mod tape_events;
use tape_events::TapeEvents;

mod tape_timeline;
use tape_timeline::TapeTimeline;

use crate::{LoadedTape, LoadedTapes};

pub struct TabViewer<'a> {
    pub tapes: &'a LoadedTapes,
    pub utc_offset: time::UtcOffset,
    pub global_time_span: std::ops::Range<time::OffsetDateTime>,
    pub selected_range: &'a mut Option<std::ops::Range<f64>>,
    pub timeline_center: &'a mut time::OffsetDateTime,
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

pub enum Tab {
    GlobalTimeline(GlobalTimeline),
    Events(TapeEvents),
    Timeline(TapeTimeline),
}

impl Tab {
    pub fn global_timeline() -> Self {
        Self::GlobalTimeline(GlobalTimeline {})
    }

    pub fn events<P: Into<PathBuf>>(tape_path: P) -> Self {
        Self::Events(TapeEvents::new(tape_path))
    }

    pub fn timeline<P: Into<PathBuf>>(tape_path: P) -> Self {
        Self::Timeline(TapeTimeline::new(tape_path))
    }
}

