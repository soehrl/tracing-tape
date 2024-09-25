use time::Duration;

use crate::{timeline::Timeline, utils::AutoColor};

use super::TabViewer;

#[derive(Default)]
pub struct GlobalTimeline {}

impl GlobalTimeline {
    pub fn id(&self) -> &str {
        "timeline"
    }

    pub fn title(&self) -> egui::WidgetText {
        "Global Timeline".into()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, viewer: &mut TabViewer) {
        let timeline = Timeline::new(
            "Global Timeline",
            Duration::ZERO..=viewer.state.timeline_duration,
        )
        .with_row_header("")
        .without_background()
        .with_selected_range(Some(viewer.state.timeline_range.clone()))
        .without_drag();

        timeline.show(ui, |timeline_ui, _| {
            let mut color_iter = AutoColor::default();
            for (level, tape) in viewer.state.loaded_tapes.iter().enumerate() {
                let start = tape.timestamp_to_global_offset(0, viewer.state.timeline_start_time);
                let span = tape.adjusted_timespan();
                let end = start + (span.end - span.start);

                timeline_ui.item(
                    level,
                    tape.path.to_string_lossy().to_string(),
                    color_iter.next().expect("color"),
                    start..=end,
                );
            }
        });
    }
}
