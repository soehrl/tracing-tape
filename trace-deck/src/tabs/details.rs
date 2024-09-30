use std::path::PathBuf;

use petgraph::graph::NodeIndex;
use time::Duration;

use crate::statistics::{
    calculate_statistics, CallsiteStatistics, EventCallsiteStatistics, SpanCallsiteStatistics,
};

use super::TabViewer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedItem {
    Callsite(usize),
    Span {
        tape: PathBuf,
        span_index: NodeIndex<usize>,
    },
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
        match &viewer.state.selected_item {
            Some(SelectedItem::Callsite(callsite)) => {
                self.callsite_ui(ui, viewer, *callsite);
            }
            Some(SelectedItem::Span { span_index, tape }) => {
                self.span_ui(ui, viewer, tape.clone(), *span_index);
            }
            None => {}
        }
    }

    fn callsite_ui(&mut self, ui: &mut egui::Ui, viewer: &mut TabViewer, callsite_index: usize) {
        let callsite = &mut viewer.state.callsites[callsite_index];

        let mut tape_statistics = vec![];

        for (path, data) in &mut callsite.tape_data {
            let statistics = data.statistics.get_or_insert_with(|| {
                calculate_statistics(
                    &viewer.state.loaded_tapes.get(&path).unwrap().tape,
                    data.callsite_index,
                )
            });
            tape_statistics.push((path, statistics));
        }

        ui.heading(callsite.inner.name().to_string());
        egui::Grid::new("callsite")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Target");
                ui.label(callsite.inner.target().to_string());
                ui.end_row();

                if let (Some(file), Some(line)) =
                    (callsite.inner.file().as_ref(), callsite.inner.line())
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
                ui.end_row();

                ui.label("Fields");
                ui.vertical(|ui| {
                    for field in callsite.inner.fields() {
                        ui.label(field.to_string());
                    }
                });
                ui.end_row();
            });

        ui.heading("Statistics");
        for (path, statistics) in tape_statistics {
            match statistics {
                CallsiteStatistics::Span(span_statistics) => {
                    Self::span_statistics_ui(ui, span_statistics, path);
                }
                CallsiteStatistics::Event(event_statistics) => {
                    Self::event_statistics_ui(ui, event_statistics, path);
                }
            }
        }
    }

    pub fn span_statistics_ui(
        ui: &mut egui::Ui,
        statistics: &SpanCallsiteStatistics,
        tape_path: &PathBuf,
    ) {
        ui.label(tape_path.to_str().unwrap());

        egui::Grid::new("span_statistics")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Calls");
                ui.label(format!("{}", statistics.span_indices.len()));
                ui.end_row();

                ui.label("Min");
                ui.label(format!("{}", Duration::nanoseconds(statistics.min)));
                ui.end_row();

                ui.label("Q1");
                ui.label(format!("{}", Duration::nanoseconds(statistics.q1)));
                ui.end_row();

                ui.label("Mean");
                ui.label(format!("{}", Duration::nanoseconds(statistics.mean)));
                ui.end_row();

                ui.label("Q2/Median");
                ui.label(format!("{}", Duration::nanoseconds(statistics.q2)));
                ui.end_row();

                ui.label("Q3");
                ui.label(format!("{}", Duration::nanoseconds(statistics.q3)));
                ui.end_row();

                ui.label("Max");
                ui.label(format!("{}", Duration::nanoseconds(statistics.max)));
                ui.end_row();

                ui.label("IQR");
                egui_plot::Plot::new("iqr")
                    .show_y(false)
                    .show_axes(false)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .allow_scroll(false)
                    .allow_boxed_zoom(false)
                    .show(ui, |ui| {
                        ui.box_plot(
                            egui_plot::BoxPlot::new(vec![egui_plot::BoxElem::new(
                                0.0,
                                egui_plot::BoxSpread {
                                    lower_whisker: statistics.min as f64,
                                    quartile1: statistics.q1 as f64,
                                    median: statistics.q2 as f64,
                                    quartile3: statistics.q3 as f64,
                                    upper_whisker: statistics.max as f64,
                                },
                            )])
                            .horizontal(),
                        );
                    })
            });
    }

    pub fn event_statistics_ui(
        ui: &mut egui::Ui,
        statistics: &EventCallsiteStatistics,
        tape_path: &PathBuf,
    ) {
    }

    fn span_ui(
        &mut self,
        ui: &mut egui::Ui,
        viewer: &mut TabViewer,
        tape: PathBuf,
        span_index: NodeIndex<usize>,
    ) {
        let tape = viewer.state.loaded_tapes.get(&tape).unwrap();
        let span = tape.tape.spans().node_weight(span_index).unwrap();
        let callsite = tape.tape.callsites().get(span.callsite_index).unwrap();

        let global_callsite_index = viewer
            .state
            .callsites
            .tape_to_global(&tape.path, span.callsite_index)
            .unwrap();

        let global_callsite = &viewer.state.callsites[global_callsite_index];

        ui.horizontal(|ui| {
            ui.heading(format!("Span {:x}", span_index.index()));
            ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);
            if ui
                .add(egui::Button::new(callsite.name()).fill(global_callsite.color))
                .clicked()
            {
                viewer.state.selected_item = Some(SelectedItem::Callsite(global_callsite_index));
            }
        });

        egui::Grid::new("span_data")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Duration");
                ui.label(format!("{}", Duration::nanoseconds(span.closed - span.opened)));
                ui.end_row();
            });

        ui.label("Fields");
        egui::Grid::new("span_fields")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                for (name, value) in callsite.fields().iter().zip(span.values.iter()) {
                    ui.label(name.to_string());
                    ui.label(value.to_string());
                    ui.end_row();
                }
            });
    }
}
