use crate::capture::CaptureRegion;
use eframe::egui::{self, CursorIcon, ViewportCommand};
use egui::vec2;

pub fn fit_to_aspect_ratio(image_size: egui::Vec2, available_size: egui::Vec2) -> egui::Vec2 {
    if image_size.x <= 0.0
        || image_size.y <= 0.0
        || available_size.x <= 0.0
        || available_size.y <= 0.0
    {
        return egui::Vec2::ZERO;
    }

    let aspect_ratio = image_size.x / image_size.y;
    let available_aspect_ratio = available_size.x / available_size.y;

    if aspect_ratio > available_aspect_ratio {
        egui::vec2(available_size.x, available_size.x / aspect_ratio)
    } else {
        egui::vec2(available_size.y * aspect_ratio, available_size.y)
    }
}

#[allow(unused_variables)]
pub fn get_scale_factor(ui: &egui::Ui) -> f32 {
    #[cfg(target_os = "windows")]
    return ui.input(|i| i.viewport().native_pixels_per_point.unwrap_or(1.0));
    #[cfg(not(target_os = "windows"))]
    1.0
}

pub fn select_window_setup(ui: &egui::Ui, region: &CaptureRegion) {
    ui.send_viewport_cmd(ViewportCommand::Decorations(false));
    ui.send_viewport_cmd(ViewportCommand::WindowLevel(
        egui::viewport::WindowLevel::AlwaysOnTop,
    ));
    if let Some(pos) = region.monitor_pos() {
        ui.send_viewport_cmd(ViewportCommand::OuterPosition(pos));
    }
    if let Some(size) = region.monitor_size() {
        ui.send_viewport_cmd(ViewportCommand::InnerSize(size - vec2(0.0, 1.0)));
    }
    ui.send_viewport_cmd(ViewportCommand::Transparent(true));
    ui.send_viewport_cmd(ViewportCommand::MousePassthrough(false));
}

#[allow(dead_code)]
pub fn close_window(ui: &egui::Ui) {
    ui.send_viewport_cmd(ViewportCommand::Close);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragInteraction {
    Body,
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl DragInteraction {
    pub fn cursor_icon(self) -> CursorIcon {
        match self {
            DragInteraction::Body => CursorIcon::Move,
            DragInteraction::TopLeft | DragInteraction::BottomRight => CursorIcon::ResizeNwSe,
            DragInteraction::TopRight | DragInteraction::BottomLeft => CursorIcon::ResizeNeSw,
            DragInteraction::Top | DragInteraction::Bottom => CursorIcon::ResizeVertical,
            DragInteraction::Left | DragInteraction::Right => CursorIcon::ResizeHorizontal,
        }
    }
}
