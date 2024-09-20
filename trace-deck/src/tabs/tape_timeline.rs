use std::path::PathBuf;

use ahash::HashMap;
use time::Duration;
use tracing_tape::Span;

use crate::state::LoadedTape;

use super::TabViewer;

#[derive(Debug, Clone, Copy, Default)]
pub struct AutoColor {
    next_auto_color_idx: u32,
}

impl Iterator for AutoColor {
    type Item = egui::Color32;

    fn next(&mut self) -> Option<Self::Item> {
        // Shamelessly copied from egui_plot::Plot::auto_color
        let i = self.next_auto_color_idx;
        self.next_auto_color_idx += 1;
        let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
        let h = i as f32 * golden_ratio;
        Some(egui::epaint::Hsva::new(h, 0.85, 0.5, 1.0).into())
    }
}

enum SpanEvent<'a> {
    Entered { exit: u64, span: &'a Span<'a> },
    Exited,
}

type ThreadSpanEvents<'a> = HashMap<u64, Vec<(u64, SpanEvent<'a>)>>;

fn get_thread_span_events<'a>(
    viewer: &TabViewer,
    loaded_tape: &'a LoadedTape,
) -> ThreadSpanEvents<'a> {
    let mut thread_span_events = HashMap::<u64, Vec<(u64, SpanEvent)>>::default();

    let timestamp_range = viewer.time_to_timestamp_span(loaded_tape, &viewer.state.timeline);

    fn ranges_overlap(a: &std::ops::Range<u64>, b: &std::ops::Range<u64>) -> bool {
        a.start < b.end && b.start < a.end
    }

    let data = loaded_tape.tape.data_for_time_span(&viewer.state.timeline);
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

    for (_, events) in &mut thread_span_events {
        events.sort_by_key(|(timestamp, _)| *timestamp);
    }

    thread_span_events
}

pub struct TapeTimeline {
    title: String,
    tape_path: PathBuf,
}

impl TapeTimeline {
    pub fn new<P: Into<PathBuf>>(tape_path: P) -> Self {
        let tape_path = tape_path.into();
        let short_filename = tape_path
            .file_name()
            .map(|f| f.to_string_lossy())
            .unwrap_or_else(|| tape_path.to_string_lossy());

        let title = format!("Timeline {}", short_filename);
        Self { title, tape_path }
    }

    pub fn id(&self) -> (&PathBuf, &str) {
        (&self.tape_path, "timeline")
    }

    pub fn title(&self) -> egui::WidgetText {
        (&self.title).into()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, viewer: &mut TabViewer) {
        let loaded_tape = if let Some(loaded_tape) = viewer.state.loaded_tapes.get(&self.tape_path)
        {
            loaded_tape
        } else {
            return;
        };

        let mut thread_span_events = get_thread_span_events(viewer, loaded_tape);

        let mut threads = loaded_tape.tape.threads().collect::<Vec<_>>();
        threads.sort_by(|(name_a, _), (name_b, _)| match name_a.cmp(name_b) {
            std::cmp::Ordering::Equal => std::cmp::Ordering::Equal,
            ordering => {
                if *name_a == "main" {
                    std::cmp::Ordering::Less
                } else if *name_b == "main" {
                    std::cmp::Ordering::Greater
                } else {
                    ordering
                }
            }
        });

        let mut color_iter = AutoColor::default();

        let mut timeline =
            crate::timeline::Timeline::new(&self.tape_path, viewer.state.timeline_range.clone());
        for (thread_name, _) in &threads {
            timeline = timeline.with_row_header(*thread_name);
        }
        let respone = timeline.show(ui, |timeline_ui, i| {
            let events = if let Some(event) = thread_span_events.get_mut(&threads[i].1) {
                event
            } else {
                return;
            };
            let color = color_iter.next().unwrap();

            let mut level = 0;
            for (timestamp, event) in events {
                match event {
                    SpanEvent::Entered { exit, span } => {
                        let callsite = loaded_tape.tape.callsite(span.callsite);

                        let response = timeline_ui.item(
                            level,
                            callsite.map(|c| c.name.to_string()).unwrap_or_default(),
                            color,
                            loaded_tape.timestamp_to_global_offset(
                                *timestamp,
                                viewer.global_time_span.start,
                            )
                                ..=loaded_tape.timestamp_to_global_offset(
                                    *exit,
                                    viewer.global_time_span.start,
                                ),
                        );
                        if let Some(callsite) = callsite {
                            let mut text = format!(
                                "{} ({:.1})\n{}",
                                callsite.name,
                                Duration::nanoseconds((*exit - *timestamp) as i64),
                                callsite.target
                            );
                            if let (Some(file), Some(line)) = (&callsite.file, callsite.line) {
                                text.push_str(&format!("\n{}:{}", file, line));
                            }
                            response.on_hover_text_at_pointer(text);
                        }
                        level += 1;
                    }
                    SpanEvent::Exited => {
                        level -= 1;
                    }
                }
            }
        });

        viewer.state.timeline_range = respone.visible_range;
    }
}
