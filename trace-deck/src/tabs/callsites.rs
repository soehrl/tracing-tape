use egui::{text::LayoutJob, TextFormat};

use super::TabViewer;

#[derive(Default)]
pub struct Callsites {
    filter: String,
}

impl Callsites {
    pub fn id(&self) -> &str {
        "callsites"
    }

    pub fn title(&self) -> egui::WidgetText {
        "Callsites".into()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, viewer: &mut TabViewer) {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Filter"));

            egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                for (index, c) in viewer
                    .state
                    .callsites
                    .iter_mut()
                    .enumerate()
                    .filter(|(_, c)| {
                        c.metadata.name.contains(&self.filter)
                            || c.metadata.target.contains(&self.filter)
                            || c.metadata
                                .file
                                .as_ref()
                                .map(|f| f.contains(&self.filter))
                                .unwrap_or(false)
                    })
                {
                    let mut job = LayoutJob::default();
                    job.append(
                        "■ ",
                        0.0,
                        TextFormat {
                            color: c.color,
                            ..Default::default()
                        },
                    );
                    job.append(&c.metadata.name, 0.0, Default::default());

                    let selected =
                        viewer.state.selected_item == Some(super::SelectedItem::Callsite(index));

                    let response = ui.selectable_label(selected, job);

                    let mut text = format!("{} ({})", c.metadata.name, c.metadata.target,);
                    if let (Some(file), Some(line)) = (c.metadata.file.as_ref(), c.metadata.line) {
                        text.push_str(&format!("\n{}:{}", file, line));
                    }

                    let response = response.on_hover_text(text);

                    if response.clicked() {
                        viewer.state.selected_item = Some(super::SelectedItem::Callsite(index));
                    }
                }
            });
        });
    }
}
