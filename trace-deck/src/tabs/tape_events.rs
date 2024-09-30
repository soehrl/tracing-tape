use std::path::PathBuf;

use egui::Align;
use egui_extras::{Column, TableBuilder};

use super::{LoadedTape, TabViewer};

pub struct TapeEvents {
    title: String,
    tape_path: PathBuf,
}

impl TapeEvents {
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
        (&self.tape_path, "events")
    }

    pub fn title(&self) -> egui::WidgetText {
        (&self.title).into()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, viewer: &mut TabViewer) {
        let loaded_tape = if let Some(tape) = viewer.state.loaded_tapes.get(&self.tape_path) {
            tape
        } else {
            return;
        };

        let available_height = ui.available_height();

        let start = loaded_tape.global_offset_to_timestamp(
            *viewer.state.timeline_range.start(),
            viewer.global_time_span.start,
        );
        let end = loaded_tape.global_offset_to_timestamp(
            *viewer.state.timeline_range.end(),
            viewer.global_time_span.start,
        );

        let events = loaded_tape.tape.events();
        let start_index = match events.binary_search_by_key(&start, |event| event.timestamp) {
            Ok(index) => index,
            Err(index) => index,
        };
        let end_index = match events.binary_search_by_key(&end, |event| event.timestamp) {
            Ok(index) => index,
            Err(index) => index,
        };

        let events_in_range = &events[start_index..end_index];

        TableBuilder::new(ui)
            .auto_shrink(false)
            .max_scroll_height(available_height)
            .column(Column::remainder())
            .cell_layout(egui::Layout::left_to_right(Align::LEFT))
            .body(|body| {
                let row_height = 18.0;
                body.rows(row_height, events_in_range.len(), move |mut row| {
                    let index = row.index();
                    row.col(|ui| {
                        let event = &events_in_range[index];
                        let callsite = &loaded_tape.tape.callsites()[event.callsite_index];

                        for (name, value) in callsite.fields().iter().zip(event.values.iter()) {
                            if &**name == "message" {
                                ui.label(format!("{value}"));
                            } else {
                                ui.label(format!("{name} = {value}"));
                            }
                        }
                    });
                });
            });
    }
}
