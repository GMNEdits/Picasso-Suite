//! Small drawn widgets: section headings.

use egui::Ui;

use crate::theme;

/// A small dim heading above a group of controls.
pub(crate) fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(11.0)
            .color(theme::TEXT_DIM)
            .strong(),
    );
    ui.add_space(2.0);
}
