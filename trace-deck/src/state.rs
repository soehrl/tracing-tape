use std::{
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use ahash::HashMap;
use time::Duration;
use tracing_tape_parser::Tape;
// use tracing_tape::Metadata;

use crate::{tabs::SelectedItem, timeline::TimeRange, utils::AutoColor};

#[derive(Debug)]
pub struct LoadedTape {
    pub path: PathBuf,
    pub tape: Tape,
    pub time_offset: time::Duration,
}

impl LoadedTape {
    pub fn adjusted_timespan(&self) -> std::ops::Range<time::OffsetDateTime> {
        let time_range = self.tape.time_range();
        let start_time = time::OffsetDateTime::from_unix_timestamp_nanos(*time_range.start())
            .unwrap()
            + self.time_offset;
        let end_time = time::OffsetDateTime::from_unix_timestamp_nanos(*time_range.end()).unwrap()
            + self.time_offset;
        start_time..end_time
    }

    pub fn timestamp_date_time(&self, timestamp: i64) -> time::OffsetDateTime {
        let base = time::OffsetDateTime::from_unix_timestamp_nanos(*self.tape.time_range().start())
            .unwrap();
        base + Duration::nanoseconds(timestamp) + self.time_offset
    }

    pub fn global_offset(&self, global_start: time::OffsetDateTime) -> time::Duration {
        let time_range = self.tape.time_range();
        let start_time =
            time::OffsetDateTime::from_unix_timestamp_nanos(*time_range.start()).unwrap();
        start_time - (global_start + self.time_offset)
    }

    pub fn timestamp_to_global_offset(
        &self,
        timestamp: i64,
        global_start: time::OffsetDateTime,
    ) -> time::Duration {
        self.global_offset(global_start) + Duration::nanoseconds(timestamp as i64)
    }

    pub fn global_offset_to_timestamp(
        &self,
        global_offset: time::Duration,
        global_start: time::OffsetDateTime,
    ) -> i64 {
        let global_offset_start = self.global_offset(global_start);
        if global_offset < global_offset_start {
            0
        } else {
            (global_offset - global_offset_start).whole_nanoseconds() as i64
        }
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
            .map(|t| t.adjusted_timespan().start)
            .min()
            .unwrap_or_else(|| time::OffsetDateTime::from_unix_timestamp(0).expect("time"));

        let t_max = self
            .iter()
            .map(|t| t.adjusted_timespan().end)
            .max()
            .unwrap_or_else(|| time::OffsetDateTime::from_unix_timestamp(0).expect("time"));

        let timeline_duration = t_max - t_min;

        State {
            current_action: Action::None,
            callsites: Callsites::for_loaded_tapes(&self),
            loaded_tapes: self,
            timeline_start_time: t_min,
            timeline_duration,
            timeline_range: Duration::ZERO..=timeline_duration,
            selected_range: None,
            selected_item: None,
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

pub struct Callsite {
    pub inner: tracing_tape_parser::Callsite,
    pub color: egui::Color32,
}

pub struct Callsites {
    callsites: Vec<Callsite>,
    tape_to_global: HashMap<(PathBuf, usize), usize>,
}

impl Callsites {
    pub fn for_loaded_tapes(tapes: &LoadedTapes) -> Self {
        // First: gather all callsites and their corresponding offset in each tape
        let mut callsites: HashMap<&tracing_tape_parser::Callsite, Vec<(&PathBuf, usize)>> =
            HashMap::default();
        for tape in &**tapes {
            for (index, callsite) in tape.tape.callsites().iter().enumerate() {
                if let Some(callsite) = callsites.get_mut(&callsite) {
                    callsite.push((&tape.path, index));
                } else {
                    callsites.insert(callsite, vec![(&tape.path, index)]);
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

        let mut color_iter = AutoColor::default();

        for (index, (callsite, tapes)) in callsites.into_iter().enumerate() {
            let callsite = Callsite {
                inner: callsite.clone(),
                color: color_iter.next().expect("color"),
            };
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

    pub fn get_for_tape(&self, path: &Path, index: usize) -> Option<&Callsite> {
        self.tape_to_global
            .get(&(path.to_path_buf(), index))
            .map(|index| &self.callsites[*index])
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
    pub timeline_start_time: time::OffsetDateTime,
    pub timeline_duration: time::Duration,
    pub timeline_range: TimeRange,
    pub selected_range: Option<TimeRange>,
    pub current_action: Action,
    pub selected_item: Option<SelectedItem>,
}
