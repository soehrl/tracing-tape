use egui_plot::PlotPoint;

pub struct Block {
    span: std::ops::Range<f64>,
    name: String,
    level: usize,
    bg: egui::Color32,
}

impl Block {
    pub fn new(
        span: std::ops::Range<f64>,
        name: String,
        level: usize,
        color: egui::Color32,
    ) -> Self {
        Self {
            span,
            name,
            level,
            bg: color,
        }
    }

    fn min(&self) -> [f64; 2] {
        [self.span.start, -(self.level as f64 + 0.9)]
    }

    fn max(&self) -> [f64; 2] {
        [self.span.end, -(self.level as f64 + 0.1)]
    }
}

impl egui_plot::PlotItem for Block {
    fn shapes(
        &self,
        ui: &egui::Ui,
        transform: &egui_plot::PlotTransform,
        shapes: &mut Vec<egui::Shape>,
    ) {
        // ui.style().visuals.faint_bg_color
        let bounds_x = transform.bounds().range_x();
        let mut visible_x = [self.min()[0], self.max()[0]];
        if visible_x[0] < *bounds_x.start() {
            visible_x[0] = *bounds_x.start();
        }
        if visible_x[1] > *bounds_x.end() {
            visible_x[1] = *bounds_x.end();
        }

        if visible_x[0] >= visible_x[1] {
            return;
        }

        let min_x = transform.position_from_point_x(*bounds_x.start());
        let max_x = transform.position_from_point_x(*bounds_x.end());

        let rect =
            transform.rect_from_values(&PlotPoint::from(self.min()), &PlotPoint::from(self.max()));

        shapes.push(egui::Shape::Rect(egui::epaint::RectShape {
            rect,
            rounding: egui::Rounding::same(2.0),
            fill: self.color(),
            stroke: Default::default(),
            blur_width: 0.0,
            fill_texture_id: Default::default(),
            uv: egui::epaint::Rect::ZERO,
        }));

        ui.fonts(|fonts| {
            let text = self.name.clone();
            let font_id = egui::FontId::default();
            let color = egui::Color32::WHITE;

            let galley = fonts.layout_no_wrap(text, font_id, color);
            if galley.rect.width() > rect.width() {
                return;
            }

            let anchor = egui::Align2::CENTER_CENTER;
            let pos = rect.center();
            let mut text_rect = anchor.anchor_size(pos, galley.size());
            if text_rect.min.x < min_x {
                let offset = f32::min(
                    min_x - text_rect.min.x,
                    (rect.width() - text_rect.width()) / 2.0,
                );
                text_rect = text_rect.translate(egui::vec2(offset, 0.0));
            }
            if text_rect.max.x > max_x {
                let offset = f32::min(
                    text_rect.max.x - max_x,
                    (rect.width() - text_rect.width()) / 2.0,
                );
                text_rect = text_rect.translate(egui::vec2(-offset, 0.0));
            }
            shapes.push(egui::Shape::galley(text_rect.min, galley, color));
        });
    }

    fn initialize(&mut self, x_range: std::ops::RangeInclusive<f64>) {
        // todo!()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn color(&self) -> egui::Color32 {
        self.bg
        // todo!()
    }

    fn highlight(&mut self) {
        // todo!()
    }

    fn highlighted(&self) -> bool {
        false
    }

    fn allow_hover(&self) -> bool {
        true
    }

    fn geometry(&self) -> egui_plot::PlotGeometry<'_> {
        egui_plot::PlotGeometry::Rects
    }

    fn bounds(&self) -> egui_plot::PlotBounds {
        egui_plot::PlotBounds::from_min_max(self.min(), self.max())
    }

    fn id(&self) -> Option<egui::Id> {
        None
    }

    fn find_closest(
        &self,
        point: egui::Pos2,
        transform: &egui_plot::PlotTransform,
    ) -> Option<egui_plot::ClosestElem> {
        let rect =
            transform.rect_from_values(&PlotPoint::from(self.min()), &PlotPoint::from(self.max()));
        let dist_sq = rect.distance_sq_to_pos(point);
        Some(egui_plot::ClosestElem { index: 0, dist_sq })
    }

    fn on_hover(
        &self,
        elem: egui_plot::ClosestElem,
        shapes: &mut Vec<egui::Shape>,
        cursors: &mut Vec<egui_plot::Cursor>,
        plot: &egui_plot::PlotConfig<'_>,
        label_formatter: &egui_plot::LabelFormatter<'_>,
    ) {
        // println!("hovering over block");
        cursors.push(egui_plot::Cursor::Vertical { x: self.min()[0] });
    }
}
