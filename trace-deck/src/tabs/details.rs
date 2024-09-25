use super::TabViewer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedItem {
    Callsite(usize),
}

pub struct Details;

impl Details {
    pub fn id(&self) -> &str {
        "details"
    }

    pub fn title(&self) -> egui::WidgetText {
        "Details".into()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, viewer: &mut TabViewer) {
        match viewer.state.selected_item {
            Some(SelectedItem::Callsite(callsite)) => {
                self.callsite_ui(ui, viewer, callsite);
            }
            None => {}
        }
    }

    fn callsite_ui(&mut self, ui: &mut egui::Ui, viewer: &mut TabViewer, callsite: usize) {
        let callsite = &mut viewer.state.callsites[callsite];
        ui.heading(callsite.inner.name.to_string());
        egui::Grid::new("callsite")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Target");
                ui.label(callsite.inner.target.to_string());
                ui.end_row();

                if let (Some(file), Some(line)) =
                    (callsite.inner.file.as_ref(), callsite.inner.line)
                {
                    ui.label("Source");
                    ui.label(format!("{}:{}", file, line));
                    ui.end_row();
                }

                ui.label("Color");
                egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut callsite.color,
                    egui::color_picker::Alpha::Opaque,
                );
            });
    }
}
