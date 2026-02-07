use std::path::PathBuf;

use super::{SelectedItem, TabViewer};

pub struct PlotSpanDuration {
    pub(super) callsite_index: usize,
    pub(super) tape: PathBuf,
}

impl PlotSpanDuration {
    pub fn id(&self) -> egui::Id {
        egui::Id::new(("plot_span_duration", &self.tape, self.callsite_index))
    }

    pub fn title(&self) -> egui::WidgetText {
        format!(
            "{} {}",
            self.callsite_index,
            self.tape.file_name().unwrap().to_string_lossy()
        )
        .into()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, viewer: &mut TabViewer) {
        let callsite = &mut viewer.state.callsites[self.callsite_index];
        let loaded_tape = viewer.state.loaded_tapes.get(&self.tape).unwrap();
        let local_callsite_index = callsite.tape_data[&self.tape].callsite_index;

        let start = loaded_tape.global_offset_to_timestamp(
            *viewer.state.timeline_range.start(),
            viewer.global_time_span.start,
        );
        let end = loaded_tape.global_offset_to_timestamp(
            *viewer.state.timeline_range.end(),
            viewer.global_time_span.start,
        );

        let spans = loaded_tape.tape.spans();

        let relevant_root_spans = loaded_tape
            .tape
            .root_spans()
            .iter()
            .filter(|span_id| {
                let span = spans.node_weight(**span_id).unwrap();
                span.opened <= end && span.closed >= start
            })
            .copied();

        let mut found_spans = vec![];

        petgraph::visit::depth_first_search(spans, relevant_root_spans, |event| {
            if let petgraph::visit::DfsEvent::Discover(span_id, _) = event {
                let span = spans.node_weight(span_id).unwrap();
                if span.closed >= start && span.opened <= end {
                    if span.callsite_index == local_callsite_index {
                        found_spans.push((span_id, span));
                        petgraph::visit::Control::<()>::Prune
                    } else {
                        petgraph::visit::Control::Continue
                    }
                } else {
                    petgraph::visit::Control::Prune
                }
            } else {
                petgraph::visit::Control::Continue
            }
        });
        found_spans.sort_unstable_by_key(|(_, span)| span.opened);

        let bars = found_spans
            .iter()
            .enumerate()
            .map(|(index, (_, span))| {
                let duration = span.closed - span.opened;
                egui_plot::Bar::new(index as f64, duration as f64)
            })
            .collect::<Vec<_>>();

        let id = egui::Id::new("bar_chart");

        let plot = egui_plot::Plot::new("span_durations")
            .allow_boxed_zoom(false)
            .allow_scroll(false)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_double_click_reset(false)
            .show(ui, |ui| {
            ui.bar_chart(egui_plot::BarChart::new("bar_chart", bars).id(id).color(callsite.color))
        });

        if plot.response.clicked() && plot.hovered_plot_item == Some(id) {
            if let Some(pos) = plot.response.hover_pos() {
                let plot_pos = plot.transform.value_from_position(pos);
                let index = plot_pos.x.round() as usize;
                if let Some((span_id, _)) = found_spans.get(index) {
                    let start = loaded_tape.timestamp_to_global_offset(
                        spans.node_weight(*span_id).unwrap().opened,
                        viewer.global_time_span.start,
                    );
                    let end = loaded_tape.timestamp_to_global_offset(
                        spans.node_weight(*span_id).unwrap().closed,
                        viewer.global_time_span.start,
                    );
                    viewer.state.timeline_range = start..=end;

                    viewer.state.selected_item = Some(SelectedItem::Span {
                        tape: self.tape.clone(),
                        span_index: *span_id,
                    });
                }
            }
        }
    }
}

