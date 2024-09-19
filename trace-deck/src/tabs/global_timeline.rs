use egui_plot::Plot;

use crate::block::Block;

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
        let color = ui.style().visuals.widgets.active.bg_fill;
        Plot::new("Global Timeline")
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .allow_double_click_reset(false)
            .allow_boxed_zoom(false)
            .include_x(0.0)
            .include_x(viewer.global_max_seconds())
            .include_y(0.0)
            .include_y(-(viewer.state.loaded_tapes.len() as f64))
            // .label_formatter(|x,_| format!("{:.3}s", x))
            .x_axis_formatter(viewer.time_axis_formatter())
            // .x_grid_spacer(viewer.time_grid_spacer())
            .y_grid_spacer(|_| vec![])
            .link_cursor("global", true, false)
            .show(ui, |plot_ui| {
                for (level, tape) in viewer.state.loaded_tapes.iter().enumerate() {
                    let b = Block::new(
                        viewer.time_to_global_span(tape.adjusted_timespan()),
                        tape.path.to_string_lossy().to_string(),
                        level,
                        color,
                    );
                    plot_ui.add(b);
                }
            });
    }
}
