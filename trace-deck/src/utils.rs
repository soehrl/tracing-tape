#[derive(Debug, Clone, Copy, Default)]
pub struct AutoColor {
    next_auto_color_idx: u32,
}

impl Iterator for AutoColor {
    type Item = egui::Color32;

    fn next(&mut self) -> Option<Self::Item> {
        // Shamelessly copied from egui_plot::Plot::auto_color
        let i = self.next_auto_color_idx;
        self.next_auto_color_idx += 1;
        let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
        let h = i as f32 * golden_ratio;
        Some(egui::epaint::Hsva::new(h, 0.85, 0.5, 1.0).into())
    }
}

