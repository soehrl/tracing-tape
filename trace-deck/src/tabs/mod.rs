use std::path::PathBuf;

mod welcome;
pub use welcome::Welcome;

mod global_timeline;
use global_timeline::GlobalTimeline;

mod tape_events;
use tape_events::TapeEvents;

mod tape_timeline;
use tape_timeline::TapeTimeline;

mod callsites;
use callsites::Callsites;

mod details;
pub use details::{Details, SelectedItem};

use crate::{state::State, LoadedTape};

pub struct TabViewer<'a> {
    pub state: &'a mut State,
    pub global_time_span: std::ops::Range<time::OffsetDateTime>,
}

impl egui_dock::TabViewer for TabViewer<'_> {
    type Tab = Tab;

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        match tab {
            Tab::Welcome(welcome) => egui::Id::new(welcome.id()),
            Tab::Callsites(callsites) => egui::Id::new(callsites.id()),
            Tab::GlobalTimeline(timeline) => egui::Id::new(timeline.id()),
            Tab::Events(tape) => egui::Id::new(tape.id()),
            Tab::Timeline(tape) => egui::Id::new(tape.id()),
            Tab::Details(details) => egui::Id::new(details.id()),
        }
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            Tab::Welcome(welcome) => welcome.title(),
            Tab::Callsites(callsites) => callsites.title(),
            Tab::GlobalTimeline(timeline) => timeline.title(),
            Tab::Events(tape) => tape.title(),
            Tab::Timeline(tape) => tape.title(),
            Tab::Details(details) => details.title(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            Tab::Welcome(welcome) => welcome.ui(ui, self),
            Tab::Callsites(callsites) => callsites.ui(ui, self),
            Tab::GlobalTimeline(timeline) => timeline.ui(ui, self),
            Tab::Events(tape) => tape.ui(ui, self),
            Tab::Timeline(tape) => tape.ui(ui, self),
            Tab::Details(details) => details.ui(ui, self),
        }
    }

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        match _tab {
            Tab::Welcome(_) => false,
            Tab::Callsites(_) => true,
            Tab::GlobalTimeline(_) => true,
            Tab::Events(_) => true,
            Tab::Timeline(_) => true,
            Tab::Details(_) => true,
        }
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        match _tab {
            Tab::Welcome(_) => false,
            Tab::Callsites(_) => true,
            Tab::GlobalTimeline(_) => true,
            Tab::Events(_) => true,
            Tab::Timeline(_) => true,
            Tab::Details(_) => true,
        }
    }
}

pub enum Tab {
    Welcome(Welcome),
    Callsites(Callsites),
    GlobalTimeline(GlobalTimeline),
    Events(TapeEvents),
    Timeline(TapeTimeline),
    Details(Details),
}

impl Tab {
    pub fn welcome() -> Self {
        Self::Welcome(Welcome)
    }

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

    pub fn details() -> Self {
        Self::Details(Details)
    }
}
