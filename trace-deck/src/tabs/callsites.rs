use crate::state::{char_from_marker_shape, CALLSITE_MARKS};

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
        ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Search"));

        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            for c in viewer.state.callsites.iter_mut().filter(|c| {
                c.metadata.name.contains(&self.filter)
                    || c.metadata.target.contains(&self.filter)
                    || c.metadata
                        .file
                        .as_ref()
                        .map(|f| f.contains(&self.filter))
                        .unwrap_or(false)
            }) {
                let text: egui::WidgetText = if let Some(marker_index) = c.mark_index {
                    let (marker, color) = CALLSITE_MARKS[marker_index];

                    egui::WidgetText::from(format!(
                        "{} ({})",
                        c.metadata.name,
                        char_from_marker_shape(marker)
                    ))
                    .color(color)
                } else {
                    c.metadata.name.clone().into()
                };

                let response = ui.selectable_label(false, text);

                let mut text = format!("{} ({})", c.metadata.name, c.metadata.target,);
                if let (Some(file), Some(line)) = (c.metadata.file.as_ref(), c.metadata.line) {
                    text.push_str(&format!("\n{}:{}", file, line));
                }

                response.on_hover_text(text);

                // Looks too ugly
                // if response.clicked() {
                //     if let Some(marker_index) = c.mark_index {
                //         c.mark_index = Some((marker_index + 1) % CALLSITE_MARKS.len());
                //     } else {
                //         c.mark_index = Some(0);
                //     }
                // }
                // if response.clicked_by(egui::PointerButton::Secondary) {
                //     c.mark_index = None;
                // }
            }
        });
    }
}
