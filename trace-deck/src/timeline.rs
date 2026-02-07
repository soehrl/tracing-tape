use std::{
    fmt::{Display, Formatter},
    sync::Arc,
};

use egui::{PointerButton, Response};

pub type Duration = time::Duration;
pub type TimeRange = std::ops::RangeInclusive<Duration>;

pub struct Timeline {
    id: egui::Id,
    visible_range: TimeRange,
    selected_range: Option<TimeRange>,
    row_headers: Vec<String>,
    allow_drag: bool,
    background: bool,
}

impl Timeline {
    pub fn new(id_source: impl std::hash::Hash, visible_range: TimeRange) -> Self {
        Self {
            id: egui::Id::new(id_source),
            visible_range,
            selected_range: None,
            row_headers: Vec::new(),
            allow_drag: true,
            background: true,
        }
    }

    pub fn with_selected_range(mut self, selected_range: Option<TimeRange>) -> Self {
        self.selected_range = selected_range;
        self
    }

    pub fn without_drag(mut self) -> Self {
        self.allow_drag = false;
        self
    }

    pub fn without_background(mut self) -> Self {
        self.background = false;
        self
    }

    pub fn with_row_header(mut self, row_header: impl Into<String>) -> Self {
        self.row_headers.push(row_header.into());
        self
    }

    pub fn show<F: FnMut(&mut TimelineUi, usize)>(
        self,
        ui: &mut egui::Ui,
        f: F,
    ) -> TimelineResponse {
        ui.push_id(self.id, |ui| self.show_impl(ui, f)).inner
    }

    fn show_impl<F: FnMut(&mut TimelineUi, usize)>(
        self,
        ui: &mut egui::Ui,
        mut f: F,
    ) -> TimelineResponse {
        let Self { row_headers, .. } = self;

        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = response.rect;

        let font_id = &ui.style().text_styles[&egui::TextStyle::Body];
        let row_header_galleys = ui.fonts_mut(|f| {
            row_headers
                .into_iter()
                .map(move |h| f.layout_no_wrap(h, font_id.clone(), egui::Color32::WHITE))
                .collect::<Vec<_>>()
        });
        let max_row_header_width = row_header_galleys
            .iter()
            .map(|g| g.rect.width())
            .fold(0.0, f32::max);

        let spacing = &ui.style().spacing;
        let header_column_width = if max_row_header_width > 0.0 {
            max_row_header_width
                + 2.0 * spacing.item_spacing.x
                + spacing.icon_width
                + spacing.icon_spacing
        } else {
            0.0
        };
        let header_rect = if header_column_width > 0.0 {
            let mut header_rect = response.rect;
            header_rect.set_width(header_column_width);
            Some(header_rect)
        } else {
            None
        };

        let axis_height = font_id.size + 2.0 * spacing.item_spacing.y;
        let mut axis_rect = rect;
        axis_rect.set_left(rect.left() + header_column_width);
        axis_rect.set_height(axis_height);

        let mut data_rect = rect;
        data_rect.set_top(axis_rect.bottom());
        if let Some(header_rect) = &header_rect {
            data_rect.set_left(header_rect.right());
        }

        let item_height = font_id.size + 2.0 * spacing.button_padding.y;

        let mut timeline_ui = TimelineUi {
            // select_mode: ui.input(|i| i.modifiers) == egui::Modifiers::SHIFT,
            select_mode: false,
            ui,
            data_painter: painter.with_clip_rect(data_rect),
            painter,
            axis_rect,
            header_rect,
            data_rect,
            visible_range: self.visible_range.clone(),
            selected_range: self.selected_range.clone(),
            text_color: egui::Color32::WHITE,
            base_offset: axis_rect.height(),
            item_height,
            rect,
            current_level: 0,
            max_level: 0,
            current_row: 0,
            background: self.background,
        };

        if self.allow_drag {
            if response.dragged_by(PointerButton::Primary) {
                let modifiers = timeline_ui.ui.input(|i| i.modifiers);

                let dragged_points = response.drag_delta().x;
                if modifiers == egui::Modifiers::NONE {
                    let dragged_duration = timeline_ui.dx2dt(dragged_points);
                    timeline_ui.visible_range = *timeline_ui.visible_range.start()
                        - dragged_duration
                        ..=*timeline_ui.visible_range.end() - dragged_duration;
                } else if modifiers == egui::Modifiers::SHIFT {
                }
            }
            if let Some(hover_pos) = response.hover_pos() {
                let (scroll, zoom) = timeline_ui
                    .ui
                    .input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));

                let hover_time = timeline_ui.x2t(hover_pos.x);
                let factor = 2.0f32.powf(-scroll * 0.01) / zoom;

                let start = *timeline_ui.visible_range.start();
                let delta_start = hover_time - start;
                let new_start = hover_time - delta_start * factor;

                let end = *timeline_ui.visible_range.end();
                let delta_end = end - hover_time;
                let new_end = hover_time + delta_end * factor;

                timeline_ui.visible_range = new_start..=new_end;
            }
        }

        timeline_ui.paint_axis();

        for (i, row_header) in row_header_galleys.into_iter().enumerate() {
            timeline_ui.begin_row(i, row_header);
            f(&mut timeline_ui, i);
            timeline_ui.end_row();
        }

        timeline_ui.paint_selection();

        TimelineResponse {
            response,
            visible_range: timeline_ui.visible_range,
            // selected_range: self.selected_range,
        }
    }
}

pub struct TimelineUi<'a> {
    ui: &'a mut egui::Ui,
    painter: egui::Painter,
    data_painter: egui::Painter,
    rect: egui::Rect,
    axis_rect: egui::Rect,
    header_rect: Option<egui::Rect>,
    data_rect: egui::Rect,
    visible_range: TimeRange,
    selected_range: Option<TimeRange>,
    text_color: egui::Color32,
    base_offset: f32,
    item_height: f32,
    current_level: usize,
    max_level: usize,
    current_row: usize,
    background: bool,
    select_mode: bool,
}

fn axis_tick_width(range: &TimeRange) -> Duration {
    let duration = *range.end() - *range.start();

    // One could employ a sophisticated algorithm to determine the best tick width or, one could
    // just brute force it...
    let tick_widths: [Duration; 38] = [
        Duration::HOUR,
        Duration::MINUTE * 30,
        Duration::MINUTE * 10,
        Duration::MINUTE * 5,
        Duration::MINUTE * 2,
        Duration::MINUTE,
        Duration::SECOND * 30,
        Duration::SECOND * 10,
        Duration::SECOND * 5,
        Duration::SECOND * 2,
        Duration::SECOND,
        Duration::MILLISECOND * 500,
        Duration::MILLISECOND * 200,
        Duration::MILLISECOND * 100,
        Duration::MILLISECOND * 50,
        Duration::MILLISECOND * 20,
        Duration::MILLISECOND * 10,
        Duration::MILLISECOND * 5,
        Duration::MILLISECOND * 2,
        Duration::MILLISECOND,
        Duration::MICROSECOND * 500,
        Duration::MICROSECOND * 200,
        Duration::MICROSECOND * 100,
        Duration::MICROSECOND * 50,
        Duration::MICROSECOND * 20,
        Duration::MICROSECOND * 10,
        Duration::MICROSECOND * 5,
        Duration::MICROSECOND * 2,
        Duration::MICROSECOND,
        Duration::NANOSECOND * 500,
        Duration::NANOSECOND * 200,
        Duration::NANOSECOND * 100,
        Duration::NANOSECOND * 50,
        Duration::NANOSECOND * 20,
        Duration::NANOSECOND * 10,
        Duration::NANOSECOND * 5,
        Duration::NANOSECOND * 2,
        Duration::NANOSECOND,
    ];

    for &tick_width in tick_widths.iter() {
        if duration / tick_width > 5.0 {
            return tick_width;
        }
    }
    Duration::NANOSECOND
}

struct TimelineAxisFormat {
    time: Duration,
    tick_width: Duration,
}

impl Display for TimelineAxisFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.tick_width >= Duration::HOUR {
            write!(f, "{}h", self.time.whole_hours())
        } else if self.tick_width >= Duration::MINUTE {
            write!(f, "{}m", self.time.whole_minutes())
        } else if self.tick_width >= Duration::SECOND {
            write!(f, "{}s", self.time.whole_seconds())
        } else if self.tick_width >= Duration::MILLISECOND {
            write!(f, "{}ms", self.time.whole_milliseconds())
        } else if self.tick_width >= Duration::MICROSECOND {
            write!(f, "{}µs", self.time.whole_microseconds())
        } else {
            write!(f, "{}ns", self.time.whole_nanoseconds())
        }
    }
}

impl TimelineUi<'_> {
    fn paint_axis(&mut self) {
        if self.background {
            let mut axis_rect = self.axis_rect;
            let axis_color = self.ui.style().visuals.widgets.inactive.bg_fill;
            axis_rect.set_left(self.rect.left());
            self.painter.rect_filled(axis_rect, 0.0, axis_color);
        }

        let font_id = &self.ui.style().text_styles[&egui::TextStyle::Body];
        self.painter.text(
            self.axis_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            format!("{:.2}", self.visible_range.start()),
            font_id.clone(),
            self.text_color,
        );

        let tick_width = axis_tick_width(&self.visible_range);
        let mut tick = *self.visible_range.start() + tick_width;
        while tick <= *self.visible_range.end() {
            let text = format!(
                "{}",
                TimelineAxisFormat {
                    time: tick - *self.visible_range.start(),
                    tick_width,
                }
            );

            let galley =
                self.painter
                    .layout_no_wrap(text.to_string(), font_id.clone(), self.text_color);
            let anchor_pos = self.ty2pos(tick, self.axis_rect.center().y);
            let rect = egui::Align2::CENTER_CENTER.anchor_size(anchor_pos, galley.size());
            let mut pos = rect.min;
            if pos.x < self.axis_rect.min.x {
                pos.x = self.axis_rect.min.x;
            }
            if pos.x + rect.width() > self.axis_rect.max.x {
                pos.x = self.axis_rect.max.x - rect.width();
            }
            self.painter.galley(pos, galley, self.text_color);

            tick += tick_width;
        }
    }

    pub fn dt2dx(&self, duration: Duration) -> f32 {
        let rel = duration / (*self.visible_range.end() - *self.visible_range.start());
        rel as f32 * self.axis_rect.width()
    }

    fn dx2dt(&self, points: f32) -> Duration {
        let rel = points / self.data_rect.width();
        (*self.visible_range.end() - *self.visible_range.start()) * rel
    }

    fn t2x(&self, t: Duration) -> f32 {
        let offset = t - *self.visible_range.start();
        let rel = offset / (*self.visible_range.end() - *self.visible_range.start());
        self.axis_rect.left() + rel as f32 * self.axis_rect.width()
    }

    fn x2t(&self, x: f32) -> Duration {
        let rel = (x - self.axis_rect.left()) / self.axis_rect.width();
        *self.visible_range.start()
            + (*self.visible_range.end() - *self.visible_range.start()) * rel
    }

    fn ty2pos(&self, t: Duration, y: f32) -> egui::Pos2 {
        let x = self.t2x(t);
        egui::Pos2::new(x, y)
    }

    fn fill_vertical(&mut self, top: f32, bottom: f32) {
        if !self.background {
            return;
        }
        let rect = egui::Rect::from_min_max(
            egui::Pos2::new(self.rect.left(), top + self.rect.top()),
            egui::Pos2::new(self.rect.right(), bottom + self.rect.top()),
        );

        let color = if self.current_row % 2 == 0 {
            self.ui
                .style()
                .visuals
                .widgets
                .inactive
                .weak_bg_fill
                .linear_multiply(0.5)
        } else {
            self.ui.style().visuals.widgets.inactive.bg_fill
        };
        self.painter.rect_filled(rect, 0.0, color);
    }

    fn begin_row(&mut self, row: usize, galley: Arc<egui::Galley>) {
        self.current_level = 0;
        self.max_level = 0;
        self.current_row = row;

        self.fill_vertical(self.base_offset, self.level_offset(1));

        if let Some(header_rect) = &self.header_rect {
            let item_spacing = self.ui.style().spacing.item_spacing;
            let icon_spacing = self.ui.style().spacing.icon_spacing;

            let mut icon_rect = *header_rect;
            icon_rect.set_width(self.ui.style().spacing.icon_width);
            icon_rect.set_height(self.ui.style().spacing.icon_width);
            let icon_rect = icon_rect.translate(egui::vec2(
                item_spacing.x,
                self.base_offset + item_spacing.y,
            ));

            // let icon_response =
            //     self.ui
            //         .interact(icon_rect, egui::Id::new(row), egui::Sense::click());
            // paint_default_icon(&mut self.ui, 0.0, &icon_response);
            let galley_pos =
                icon_rect.right_center() + egui::vec2(icon_spacing, -galley.rect.height() * 0.5);
            self.painter.galley(galley_pos, galley, self.text_color);
        }
    }

    fn end_row(&mut self) {
        self.base_offset += (self.max_level + 1) as f32
            * (self.item_height + self.ui.style().spacing.item_spacing.y)
            + self.ui.style().spacing.item_spacing.y;
    }

    fn level_offset(&self, level: usize) -> f32 {
        self.base_offset
            + level as f32 * (self.item_height + self.ui.style().spacing.item_spacing.y)
            + self.ui.style().spacing.item_spacing.y
    }

    pub fn item(
        &mut self,
        level: usize,
        text: String,
        color: egui::Color32,
        span: TimeRange,
    ) -> Response {
        let level_offset = self.level_offset(level);
        if level > self.max_level {
            self.fill_vertical(
                self.level_offset(self.max_level + 1),
                self.level_offset(level + 1),
            );
            self.max_level = level;
        }

        let top = self.rect.top() + level_offset + self.ui.style().spacing.button_padding.y;
        let bottom = top + self.item_height;

        let rect = egui::Rect::from_min_max(
            egui::Pos2::new(self.t2x(*span.start()), top),
            egui::Pos2::new(self.t2x(*span.end()), bottom),
        );

        let response = self.ui.interact(
            rect,
            egui::Id::new((self.current_row, level, span)),
            egui::Sense::click(),
        );
        let visuals = self.ui.style().noninteractive();
        let rounding = visuals.corner_radius;
        self.data_painter.rect_filled(
            rect,
            rounding,
            if self.select_mode {
                let mut hsva = egui::epaint::Hsva::from_srgba_premultiplied(color.to_array());
                hsva.s *= 0.5;
                hsva.into()
            } else {
                color
            },
        );
        if rect.width() > 10.0 {
            self.text(&rect, text);
        }

        // let width = 9.0;
        // if self.select_mode && rect.width() >= 2.0 * width {
        //     let shadow = egui::Shadow {
        //         offset: egui::vec2(0.0, 0.0),
        //         color: egui::Color32::WHITE,
        //         blur: 2.0,
        //         spread: 2.0,
        //     };
        //     {
        //         let mut rect = rect;
        //         rect.set_width(width);

        //         let mut rounding = rounding;
        //         rounding.ne = 0.0;
        //         rounding.se = 0.0;

        //         self.data_painter.add(shadow.as_shape(rect, rounding));
        //         self.data_painter.rect_filled(rect, rounding, color);
        //     }
        //     {
        //         let mut rect = rect;
        //         rect.set_left(rect.right() - width);

        //         let mut rounding = rounding;
        //         rounding.nw = 0.0;
        //         rounding.sw = 0.0;

        //         self.data_painter.add(shadow.as_shape(rect, rounding));
        //         self.data_painter.rect_filled(rect, rounding, color);
        //     }
        // }

        response
    }

    fn text(&mut self, rect: &egui::Rect, text: String) {
        let text_padding = self.ui.style().spacing.button_padding;

        if let Some(galley) = self.ui.fonts_mut(|fonts| {
            let font_id = egui::FontId::default();
            let galley = fonts.layout_no_wrap(text, font_id, self.text_color);
            if galley.rect.width() + text_padding.x * 2.0 > rect.width() {
                return None;
            } else {
                return Some(galley);
            }
        }) {
            let anchor = egui::Align2::CENTER_CENTER;
            let pos = rect.center();
            let mut text_rect = anchor.anchor_size(pos, galley.size() + 2.0 * text_padding);
            if text_rect.min.x < self.data_rect.min.x {
                let offset = f32::min(
                    self.data_rect.min.x - text_rect.min.x,
                    (rect.width() - text_rect.width()) / 2.0,
                );
                text_rect = text_rect.translate(egui::vec2(offset, 0.0));
            }
            if text_rect.max.x > self.data_rect.max.x {
                let offset = f32::min(
                    text_rect.max.x - self.data_rect.max.x,
                    (rect.width() - text_rect.width()) / 2.0,
                );
                text_rect = text_rect.translate(egui::vec2(-offset, 0.0));
            }
            self.data_painter.galley_with_override_text_color(
                text_rect.min + text_padding,
                galley,
                self.text_color,
            );
        }
    }

    fn paint_selection(&mut self) {
        if let Some(selected_range) = &self.selected_range {
            let top = self.rect.top();
            let bottom = self.rect.bottom();
            let left = self.data_rect.left();
            let right = self.t2x(*selected_range.start());
            let rect = egui::Rect::from_min_max(
                egui::Pos2::new(left, top),
                egui::Pos2::new(right, bottom),
            );
            self.painter
                .rect_filled(rect, 0.0, egui::Color32::from_black_alpha(64));

            let left = self.t2x(*selected_range.end());
            let right = self.data_rect.right();
            let rect = egui::Rect::from_min_max(
                egui::Pos2::new(left, top),
                egui::Pos2::new(right, bottom),
            );
            self.painter
                .rect_filled(rect, 0.0, egui::Color32::from_black_alpha(64));
        }
    }
}

pub struct TimelineResponse {
    pub response: egui::Response,
    pub visible_range: TimeRange,
}
