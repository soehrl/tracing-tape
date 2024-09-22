use std::{
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use ahash::HashMap;
use time::Duration;
use trace_deck::Tape;
use tracing_tape::Metadata;

use crate::timeline::TimeRange;

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

    pub fn timestamp_date_time(&self, timestamp: u64) -> time::OffsetDateTime {
        self.tape.timestamp_date_time(timestamp) + self.time_offset
    }

    pub fn global_offset(&self, global_start: time::OffsetDateTime) -> time::Duration {
        self.tape.time_span().start - (global_start + self.time_offset)
    }

    pub fn timestamp_to_global_offset(
        &self,
        timestamp: u64,
        global_start: time::OffsetDateTime,
    ) -> time::Duration {
        self.global_offset(global_start) + Duration::nanoseconds(timestamp as i64)
    }
}

#[derive(Debug, Default)]
pub struct LoadedTapes(Vec<LoadedTape>);

impl From<Vec<LoadedTape>> for LoadedTapes {
    fn from(tapes: Vec<LoadedTape>) -> Self {
        Self(tapes)
    }
}

impl Into<State> for Vec<LoadedTape> {
    fn into(self) -> State {
        <_ as Into<State>>::into(<_ as Into<LoadedTapes>>::into(self))
    }
}

impl Into<State> for LoadedTapes {
    fn into(self) -> State {
        let t_min = self
            .iter()
            .map(|t| t.tape.time_span().start)
            .min()
            .unwrap_or_else(time::OffsetDateTime::now_utc);
        let t_max = self
            .iter()
            .map(|t| t.tape.time_span().end)
            .max()
            .unwrap_or_else(time::OffsetDateTime::now_utc);
        println!("t_min: {:?}, t_max: {:?}", t_min, t_max);
        State {
            current_action: Action::None,
            callsites: Callsites::for_loaded_tapes(&self),
            timeline: t_min..t_max,
            loaded_tapes: self,
            timeline_range: Duration::ZERO..=(t_max - t_min),
            selected_range: None,
        }
    }
}

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

pub const CALLSITE_MARKS: [(egui_plot::MarkerShape, egui::Color32); 10] = [
    (egui_plot::MarkerShape::Circle, egui::Color32::YELLOW),
    (egui_plot::MarkerShape::Diamond, egui::Color32::DARK_RED),
    (egui_plot::MarkerShape::Square, egui::Color32::DARK_GREEN),
    (egui_plot::MarkerShape::Cross, egui::Color32::DARK_BLUE),
    (
        egui_plot::MarkerShape::Plus,
        egui::Color32::from_rgb(255, 20, 147),
    ),
    (egui_plot::MarkerShape::Up, egui::Color32::GREEN),
    (egui_plot::MarkerShape::Down, egui::Color32::RED),
    (egui_plot::MarkerShape::Left, egui::Color32::GOLD),
    (egui_plot::MarkerShape::Right, egui::Color32::BLUE),
    (egui_plot::MarkerShape::Asterisk, egui::Color32::BROWN),
];

pub fn char_from_marker_shape(shape: egui_plot::MarkerShape) -> char {
    match shape {
        egui_plot::MarkerShape::Circle => '●',
        egui_plot::MarkerShape::Diamond => '◆',
        egui_plot::MarkerShape::Square => '■',
        egui_plot::MarkerShape::Cross => 'x',
        egui_plot::MarkerShape::Plus => '+',
        egui_plot::MarkerShape::Up => '⏶',
        egui_plot::MarkerShape::Down => '⏷',
        egui_plot::MarkerShape::Left => '⏴',
        egui_plot::MarkerShape::Right => '⏵',
        egui_plot::MarkerShape::Asterisk => '*',
    }
}

pub struct Callsite {
    pub metadata: Metadata<'static>,
    pub mark_index: Option<usize>,
}

impl From<&Metadata<'_>> for Callsite {
    fn from(metadata: &Metadata) -> Self {
        Self {
            metadata: metadata.to_static(),
            mark_index: None,
        }
    }
}

pub struct Callsites {
    callsites: Vec<Callsite>,
    tape_to_global: HashMap<(PathBuf, u64), usize>,
}

impl Callsites {
    pub fn for_loaded_tapes(tapes: &LoadedTapes) -> Self {
        // First: gather all callsites and their corresponding offset in each tape
        let mut callsites: HashMap<&Metadata, Vec<(&PathBuf, u64)>> = HashMap::default();
        for tape in &**tapes {
            for (offset, metadata) in tape.tape.callsites() {
                if let Some(callsite) = callsites.get_mut(&metadata) {
                    callsite.push((&tape.path, *offset));
                } else {
                    callsites.insert(metadata, vec![(&tape.path, *offset)]);
                }
            }
        }

        // Then sort them by target>filename>line>name
        let mut callsites = callsites.drain().collect::<Vec<_>>();
        callsites.sort_by(|(a, _), (b, _)| {
            a.target.cmp(&b.target).then(
                a.file
                    .cmp(&b.file)
                    .then(a.line.cmp(&b.line))
                    .then(a.name.cmp(&b.name)),
            )
        });

        let mut tape_to_global = HashMap::default();
        let mut callsite_vec = Vec::with_capacity(callsites.len());

        for (index, (metadata, tapes)) in callsites.into_iter().enumerate() {
            let callsite = Callsite::from(metadata);
            callsite_vec.push(callsite);

            for (path, offset) in tapes {
                tape_to_global.insert((path.clone(), offset), index);
            }
        }

        Self {
            callsites: callsite_vec,
            tape_to_global,
        }
    }
}

impl Deref for Callsites {
    type Target = Vec<Callsite>;

    fn deref(&self) -> &Self::Target {
        &self.callsites
    }
}

impl DerefMut for Callsites {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.callsites
    }
}

pub enum Action {
    None,
    Measure { from: time::Duration },
}

pub struct State {
    pub loaded_tapes: LoadedTapes,
    pub callsites: Callsites,
    pub timeline: std::ops::Range<time::OffsetDateTime>,
    pub timeline_range: TimeRange,
    pub selected_range: Option<TimeRange>,
    pub current_action: Action,
}
