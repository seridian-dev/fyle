//! Shared action-button treatment for Fyle's neon palette.

use crate::theme::colors;
use eframe::egui::{self, Color32, RichText, Stroke};

pub fn accent(ui: &mut egui::Ui, label: impl Into<RichText>, color: Color32) -> egui::Response {
    let response = ui.add(
        egui::Button::new(label.into().strong().color(colors::TEXT))
            .fill(color.gamma_multiply(0.22))
            .stroke(Stroke::new(1.0_f32, color)),
    );
    if response.hovered() {
        ui.painter().rect_stroke(
            response.rect.expand(1.0),
            4.0,
            Stroke::new(1.0_f32, color.gamma_multiply(0.72)),
        );
    }
    response
}

pub fn enabled(
    ui: &mut egui::Ui,
    label: impl Into<RichText>,
    color: Color32,
    enabled: bool,
) -> egui::Response {
    let button = if enabled {
        egui::Button::new(label.into().strong().color(colors::TEXT))
            .fill(color.gamma_multiply(0.22))
            .stroke(Stroke::new(1.0_f32, color))
    } else {
        egui::Button::new(label.into().color(colors::MUTED))
            .fill(colors::DARK_SLATE)
            .stroke(Stroke::new(1.0_f32, colors::BORDER))
    };
    ui.add_enabled(enabled, button)
}

pub fn subtle(ui: &mut egui::Ui, label: impl Into<RichText>) -> egui::Response {
    let response = ui.add(
        egui::Button::new(label.into().color(colors::TEXT))
            .fill(colors::MIDNIGHT_PURPLE)
            .stroke(Stroke::new(1.0_f32, colors::BORDER)),
    );
    if response.hovered() {
        ui.painter().rect_stroke(
            response.rect.expand(1.0),
            4.0,
            Stroke::new(1.0_f32, colors::CYAN),
        );
    }
    response
}

pub fn compact(ui: &mut egui::Ui, label: impl Into<RichText>) -> egui::Response {
    let response = ui.add(
        egui::Button::new(label.into().color(colors::TEXT))
            .small()
            .fill(colors::MIDNIGHT_PURPLE)
            .stroke(Stroke::new(1.0_f32, colors::BORDER)),
    );
    if response.hovered() {
        ui.painter().rect_stroke(
            response.rect.expand(1.0),
            4.0,
            Stroke::new(1.0_f32, colors::CYAN),
        );
    }
    response
}
