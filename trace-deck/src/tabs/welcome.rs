use super::TabViewer;

pub struct Welcome;

impl Welcome {
    pub fn id(&self) -> &str {
        "welcome"
    }

    pub fn title(&self) -> egui::WidgetText {
        "Welcome".into()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, _viewer: &mut TabViewer) {
        ui.heading("Welcome");
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            ui.label("Drop files generated by the tracing tape recorder here to get started.");
            // ui.hyperlink_to("tracing tape recorder", "https://docs.rs/tracing-tape-recorder/");
            // ui.label(" here to get started.");
        });
    }
}
