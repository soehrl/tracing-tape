use std::path::PathBuf;

mod welcome;

mod global_timeline;
use global_timeline::GlobalTimeline;

mod tape_events;
use tape_events::TapeEvents;

mod tape_timeline;
use tape_timeline::TapeTimeline;

mod callsites;
use callsites::Callsites;

use crate::{state::State, LoadedTape};

pub struct TabViewer<'a> {
    pub state: &'a mut State,
    pub global_time_span: std::ops::Range<time::OffsetDateTime>,
}

impl egui_dock::TabViewer for TabViewer<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        match tab {
            Tab::Callsites(callsites) => egui::Id::new(callsites.id()),
            Tab::GlobalTimeline(timeline) => egui::Id::new(timeline.id()),
            Tab::Events(tape) => egui::Id::new(tape.id()),
            Tab::Timeline(tape) => egui::Id::new(tape.id()),
        }
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            Tab::Callsites(callsites) => callsites.title(),
            Tab::GlobalTimeline(timeline) => timeline.title(),
            Tab::Events(tape) => tape.title(),
            Tab::Timeline(tape) => tape.title(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            Tab::Callsites(callsites) => callsites.ui(ui, self),
            Tab::GlobalTimeline(timeline) => timeline.ui(ui, self),
            Tab::Events(tape) => tape.ui(ui, self),
            Tab::Timeline(tape) => tape.ui(ui, self),
        }
    }
}

pub enum Tab {
    Callsites(Callsites),
    GlobalTimeline(GlobalTimeline),
    Events(TapeEvents),
    Timeline(TapeTimeline),
}

impl Tab {
    pub fn callsites() -> Self {
        Self::Callsites(Callsites::default())
    }

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
