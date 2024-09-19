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
        let LoadedTape { tape, time_offset, .. } = if let Some(tape) = viewer.tapes.get(&self.tape_path)
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
