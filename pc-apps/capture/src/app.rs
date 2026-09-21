use device_query::{DeviceQuery, DeviceState};
use eframe::egui::{self, ViewportCommand};
use egui::{
    Align2, Area, Button, CentralPanel, Color32, Frame, Key, Margin, Panel, Pos2, Rect, RichText,
    Sense, Stroke, StrokeKind, Vec2, Visuals, Window, pos2, vec2,
};
use host_utils::serial_comm::ConnectionStatus;
use protocols::capture::CaptureRequestToRuntime;
use std::sync::{Arc, Mutex, mpsc};
use xcap::Monitor;

use crate::capture::{Capture, CaptureConfig, CaptureRegion, FilterMode, process_frame_bg};
use crate::comm::{AppComm, ConnectionType};
use crate::net_comm::NetComm;
use crate::settings::{CaptureArea, CaptureSettings, get_settings_path};
use crate::ui::{DragInteraction, fit_to_aspect_ratio, get_scale_factor, select_window_setup};
use crate::usb_comm::UsbComm;
use crate::ws_comm::WsComm;

pub const LINUX_CDC_ACM_VID: u16 = 0x0525;
pub const LINUX_CDC_ACM_PID: u16 = 0xA4A7;

pub const DISPLAY_WIDTH: u16 = 48;
pub const DISPLAY_HEIGHT: u16 = 32;

pub enum State {
    Initializing {
        center_pos: Option<Pos2>,
        capture_region: Option<CaptureRegion>,
    },
    PreSelecting {
        center_pos: Pos2,
        capture_region: CaptureRegion,
        saved_selection: Option<Rect>,
    },
    Selecting {
        selection: Rect,
        dragging: Option<DragInteraction>,
        capture_region: CaptureRegion,
    },
    Capturing,
}

impl Default for State {
    fn default() -> Self {
        Self::Initializing {
            center_pos: None,
            capture_region: None,
        }
    }
}

impl State {
    pub fn transit_to_initializing(
        &mut self,
        center_pos: Option<Pos2>,
        capture_region: Option<CaptureRegion>,
    ) {
        *self = State::Initializing {
            center_pos,
            capture_region,
        };
    }

    pub fn transit_to_pre_selecting(
        &mut self,
        center_pos: Pos2,
        capture_region: CaptureRegion,
        saved_selection: Option<Rect>,
    ) {
        *self = State::PreSelecting {
            center_pos,
            capture_region,
            saved_selection,
        };
    }

    pub fn transit_to_selecting(
        &mut self,
        center_pos: Pos2,
        capture_region: CaptureRegion,
        saved_selection: Option<Rect>,
    ) {
        *self = State::Selecting {
            selection: saved_selection.unwrap_or_else(|| {
                Rect::from_center_size(center_pos, CaptureApp::INITIAL_SELECTION_SIZE)
            }),
            dragging: None,
            capture_region,
        };
    }
}

pub struct CaptureApp {
    pub state: State,
    pub capture_min_window: Vec2,
    pub app_comm: Option<AppComm>,
    pub bits_per_pixel: u8,
    pub connection_status: ConnectionStatus,
    pub connection_type: ConnectionType,
    pub serial_address: String,
    pub network_address: String,
    pub websocket_address: String,
    pub baud_rate: u32,
    pub saved_filter_mode: FilterMode,
    pub saved_invert_active: bool,
    pub saved_capture_area: Option<(i32, i32, u32, u32)>,
    pub saved_threshold: u8,
    pub available_serial_ports: Vec<String>,
    pub last_port_refresh: Option<std::time::Instant>,
    pub show_settings_window: bool,

    // Wi-Fi settings state
    pub wifi_ssids: Vec<protocols::capture::WifiNetwork>,
    pub wifi_scanning: bool,
    pub selected_ssid: String,
    pub wifi_passphrase: String,
    pub wifi_connecting: bool,
    pub wifi_connect_result: Option<Result<String, String>>,
    pub wifi_disconnect_result: Option<Result<String, String>>,
    pub wifi_ips: Vec<String>,
    pub device_state: DeviceState,
    pub hover_start_time: Option<std::time::Instant>,
    pub portrait_mode: bool,
    pub capture: Option<Capture>,
    pub capture_state: bool,
    pub use_high_range: bool,
}

impl CaptureApp {
    pub const INITIAL_SELECTION_SIZE: Vec2 = vec2(150.0, 100.0);

    pub fn transit_to_capturing(
        &mut self,
        capture_region: CaptureRegion,
        bits_per_pixel: u8,
        filter_mode: FilterMode,
        invert_active: bool,
        portrait_mode: bool,
    ) {
        let (tx_res, rx_res) = mpsc::sync_channel(2);
        let config = Arc::new(Mutex::new(CaptureConfig {
            region: Some(capture_region.clone()),
            filter_mode,
            invert_active,
            bits_per_pixel,
            capture_state: true,
            portrait_mode,
            threshold: self.saved_threshold,
            use_high_range: self.use_high_range,
        }));
        let config_clone = config.clone();
        let (tx_stop, rx_stop) = mpsc::channel();

        std::thread::spawn(move || {
            let mut current_monitor: Option<Monitor> = None;
            let mut current_monitor_id = None;
            let frame_duration = std::time::Duration::from_millis(66); // ~15 FPS (1000 / 15 = 66.6ms)
            loop {
                let start_time = std::time::Instant::now();
                if rx_stop.try_recv().is_ok() {
                    break;
                }
                let cfg = match config_clone.lock() {
                    Ok(guard) => guard.clone(),
                    Err(e) => {
                        log::error!("Capture config mutex poisoned: {}", e);
                        break;
                    }
                };
                if cfg.capture_state
                    && let Some(region) = &cfg.region
                {
                    if current_monitor_id != region.monitor_id {
                        current_monitor = Monitor::all()
                            .unwrap_or_default()
                            .into_iter()
                            .find(|m| m.id().ok() == region.monitor_id);
                        current_monitor_id = region.monitor_id;
                    }
                    if let Some(monitor) = &current_monitor
                        && let Some(res) = process_frame_bg(&cfg, monitor)
                    {
                        let _ = tx_res.try_send(res);
                    }
                }

                let elapsed = start_time.elapsed();
                if elapsed < frame_duration {
                    std::thread::sleep(frame_duration - elapsed);
                } else {
                    std::thread::yield_now();
                }
            }
        });

        self.capture_state = true;
        self.capture = Some(Capture {
            region: Some(capture_region),
            texture: None,
            filter_mode,
            invert_active,
            rx_res: Some(rx_res),
            tx_stop: Some(tx_stop),
            config: Some(config),
            window_pos_initialized: false,
            threshold: self.saved_threshold,
            device_pixels: None,
        });
        self.state = State::Capturing;
    }

    pub fn update_capture(&mut self, ctx: &egui::Context) {
        if let Some(ref mut capture) = self.capture {
            self.capture_state = true;
            if let Some(config_mutex) = &capture.config
                && let Ok(mut cfg) = config_mutex.lock()
            {
                cfg.region = capture.region.clone();
                cfg.filter_mode = capture.filter_mode;
                cfg.invert_active = capture.invert_active;
                cfg.capture_state = self.capture_state;
                cfg.bits_per_pixel = self.bits_per_pixel;
                cfg.threshold = capture.threshold;
                cfg.use_high_range = self.use_high_range;
            }

            if self.capture_state
                && let Some(rx) = &capture.rx_res
            {
                let mut latest_res = None;
                while let Ok(res) = rx.try_recv() {
                    latest_res = Some(res);
                }
                if let Some(res) = latest_res {
                    if res.color_image.size[0] > 0 && res.color_image.size[1] > 0 {
                        if let Some(texture) = &mut capture.texture {
                            texture.set(res.color_image, Default::default());
                        } else {
                            capture.texture = Some(ctx.load_texture(
                                "captured_image",
                                res.color_image,
                                Default::default(),
                            ));
                        }
                    }
                    capture.device_pixels = Some(res.device_pixels);
                    if let Some(app_comm) = &mut self.app_comm {
                        let request = CaptureRequestToRuntime::UpdateDisplay {
                            width: DISPLAY_WIDTH,
                            height: DISPLAY_HEIGHT,
                            data: res.packed_data,
                            bits_per_pixel: self.bits_per_pixel,
                        };
                        let mut usb_comm_removed = false;
                        if let Err(e) = app_comm.send_request(&request) {
                            log::error!("Failed to send request: {}", e);
                            if e.kind() == std::io::ErrorKind::BrokenPipe {
                                usb_comm_removed = true;
                            }
                        }
                        if usb_comm_removed {
                            self.app_comm = None;
                        }
                    }
                    ctx.request_repaint();
                }
            }
        }
    }

    fn save_to_file(&self) {
        let conn_type_str = match self.connection_type {
            ConnectionType::Serial => "Serial",
            ConnectionType::Network => "Network",
            ConnectionType::WebSocket => "WebSocket",
        };
        let filter_mode_str = match self.saved_filter_mode {
            FilterMode::None => "None",
            FilterMode::HighContrast => "HighContrast",
            FilterMode::Outline => "Outline",
        };

        let capture_area = self
            .saved_capture_area
            .map(|(x, y, w, h)| CaptureArea { x, y, w, h });
        let settings = CaptureSettings {
            serial_address: self.serial_address.clone(),
            network_address: self.network_address.clone(),
            websocket_address: self.websocket_address.clone(),
            connection_type: conn_type_str.to_string(),
            filter_mode: filter_mode_str.to_string(),
            invert_active: self.saved_invert_active,
            capture_area,
            threshold: self.saved_threshold,
            portrait_mode: self.portrait_mode,
            use_high_range: self.use_high_range,
        };
        if let Ok(data) = toml::to_string_pretty(&settings) {
            let _ = std::fs::write(get_settings_path(), data);
        }
    }

    fn ui_initializing(&mut self, ui: &mut egui::Ui) {
        let style = ui.style();
        let button_padding = style.spacing.button_padding;
        let item_spacing = style.spacing.item_spacing;
        let panel_margin = style.spacing.window_margin;
        let font_id = egui::TextStyle::Button.resolve(style);

        let texts = ["↩", "Quit", "⏸", "Invert", "Filter"];
        let mut side_panel_content_w = 0.0;
        let mut max_button_h = 0.0;

        for (i, text) in texts.iter().enumerate() {
            let galley = ui.painter().layout_no_wrap(
                text.to_string(),
                font_id.clone(),
                egui::Color32::WHITE,
            );
            let button_w = galley.size().x + button_padding.x * 2.0;
            let button_h = galley.size().y + button_padding.y * 2.0;
            side_panel_content_w += button_w;
            if i > 0 {
                side_panel_content_w += item_spacing.x;
            }
            if button_h > max_button_h {
                max_button_h = button_h;
            }
        }

        let min_window_w =
            side_panel_content_w + panel_margin.left as f32 + panel_margin.right as f32 + 450.0;
        let min_window_h =
            max_button_h + panel_margin.top as f32 + panel_margin.bottom as f32 + 4.0;
        self.capture_min_window = vec2(min_window_w, min_window_h);

        let (stored_center, stored_region) = if let State::Initializing {
            center_pos,
            capture_region,
        } = &self.state
        {
            (*center_pos, capture_region.clone())
        } else {
            (None, None)
        };

        CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical_centered_justified(|ui| {
                let mut force_region = None;
                let mut is_area_valid = false;
                if let Some((abs_x, abs_y, w, h)) = self.saved_capture_area {
                    let sf = get_scale_factor(ui);
                    let px = (abs_x as f32 * sf).round() as i32;
                    let py = (abs_y as f32 * sf).round() as i32;

                    if Monitor::from_point(px, py).is_ok() {
                        is_area_valid = true;
                        let mut r = crate::capture::CaptureRegion::default();
                        r.select_monitor(ui, abs_x, abs_y);
                        if let Some(m_pos) = r.monitor_pos() {
                            let rel_x = (abs_x - m_pos.x as i32) as f32;
                            let rel_y = (abs_y - m_pos.y as i32) as f32;
                            force_region = Some((
                                r,
                                Rect::from_min_size(pos2(rel_x, rel_y), vec2(w as f32, h as f32)),
                            ));
                        }
                    }
                }

                if self.saved_capture_area.is_some() && !is_area_valid {
                    self.saved_capture_area = None;
                    self.save_to_file();
                }

                let center_pos = stored_center.unwrap_or_else(|| {
                    ui.input(|i| i.viewport().monitor_size)
                        .map(|size| pos2(size.x / 2.0, size.y / 2.0))
                        .unwrap_or_else(|| ui.available_rect_before_wrap().center())
                });

                let region = stored_region
                    .or_else(|| force_region.clone().map(|(r, _)| r))
                    .unwrap_or_else(|| {
                        let absolute_center = ui
                            .input(|i| i.viewport().inner_rect)
                            .map(|r| r.center())
                            .unwrap_or(center_pos);
                        let mut r = crate::capture::CaptureRegion::default();
                        r.select_monitor(ui, absolute_center.x as i32, absolute_center.y as i32);
                        r
                    });

                let saved_selection = force_region.map(|(_, s)| s);
                select_window_setup(ui, &region);
                self.state
                    .transit_to_pre_selecting(center_pos, region, saved_selection);
            });
        });
    }

    fn ui_pre_selecting(&mut self, ui: &mut egui::Ui) {
        let transition = if let State::PreSelecting {
            center_pos,
            ref capture_region,
            saved_selection,
        } = self.state
        {
            select_window_setup(ui, capture_region);
            Some((center_pos, capture_region.clone(), saved_selection))
        } else {
            None
        };

        if let Some((pos, region, saved_selection)) = transition {
            self.state
                .transit_to_selecting(pos, region, saved_selection);
        }
    }

    fn ui_selecting(&mut self, ui: &mut egui::Ui) {
        let mut transition_to_capturing = None;
        let mut transition_to_initializing = false;
        let mut do_save_area = None;
        let ratio = if self.portrait_mode { 1.0 / 1.5 } else { 1.5 };

        if let State::Selecting {
            ref mut selection,
            ref mut dragging,
            ref mut capture_region,
        } = self.state
        {
            if let Some(interaction) = *dragging {
                let mut new_selection = *selection;
                let mouse_delta = ui.input(|i| i.pointer.delta());
                let content_rect = ui.content_rect();
                ui.set_cursor_icon(interaction.cursor_icon());
                match interaction {
                    DragInteraction::Body => {
                        new_selection = new_selection.translate(mouse_delta);
                        if let Some(Pos2 { x, y }) = ui.input(|i| i.pointer.hover_pos())
                            && let Some(Pos2 { x: mx, y: my }) = capture_region.monitor_pos()
                            && !capture_region.monitor_contains((x + mx) as i32, (y + my) as i32)
                        {
                            capture_region.select_monitor(ui, (x + mx) as i32, (y + my) as i32);
                            select_window_setup(ui, capture_region);
                            return;
                        }
                        let mut offset = Vec2::ZERO;
                        if new_selection.left() < content_rect.left() {
                            offset.x = content_rect.left() - new_selection.left();
                        } else if new_selection.right() > content_rect.right() {
                            offset.x = content_rect.right() - new_selection.right();
                        }
                        if new_selection.top() < content_rect.top() {
                            offset.y = content_rect.top() - new_selection.top();
                        } else if new_selection.bottom() > content_rect.bottom() {
                            offset.y = content_rect.bottom() - new_selection.bottom();
                        }
                        new_selection = new_selection.translate(offset);
                    }
                    DragInteraction::TopLeft => {
                        let max_w = selection.max.x - content_rect.left();
                        let max_h = selection.max.y - content_rect.top();
                        let mut new_w = (selection.width() - mouse_delta.x).max(30.0);
                        let mut new_h = (selection.height() - mouse_delta.y).max(20.0);
                        if new_w > new_h * ratio {
                            new_h = new_w / ratio;
                        } else {
                            new_w = new_h * ratio;
                        }
                        if new_w > max_w {
                            new_w = max_w;
                            new_h = new_w / ratio;
                        }
                        if new_h > max_h {
                            new_h = max_h;
                            new_w = new_h * ratio;
                        }
                        new_selection.min.x = new_selection.max.x - new_w;
                        new_selection.min.y = new_selection.max.y - new_h;
                    }
                    DragInteraction::Top => {
                        let max_w = (selection.center().x - content_rect.left())
                            .min(content_rect.right() - selection.center().x)
                            * 2.0;
                        let max_h = selection.max.y - content_rect.top();
                        let mut new_h = (selection.height() - mouse_delta.y).max(20.0);
                        let mut new_w = new_h * ratio;
                        if new_w > max_w {
                            new_w = max_w;
                            new_h = new_w / ratio;
                        }
                        if new_h > max_h {
                            new_h = max_h;
                            new_w = new_h * ratio;
                        }
                        let cx = selection.center().x;
                        new_selection.min.y = new_selection.max.y - new_h;
                        new_selection.min.x = cx - new_w / 2.0;
                        new_selection.max.x = cx + new_w / 2.0;
                    }
                    DragInteraction::TopRight => {
                        let max_w = content_rect.right() - selection.min.x;
                        let max_h = selection.max.y - content_rect.top();
                        let mut new_w = (selection.width() + mouse_delta.x).max(30.0);
                        let mut new_h = (selection.height() - mouse_delta.y).max(20.0);
                        if new_w > new_h * ratio {
                            new_h = new_w / ratio;
                        } else {
                            new_w = new_h * ratio;
                        }
                        if new_w > max_w {
                            new_w = max_w;
                            new_h = new_w / ratio;
                        }
                        if new_h > max_h {
                            new_h = max_h;
                            new_w = new_h * ratio;
                        }
                        new_selection.max.x = new_selection.min.x + new_w;
                        new_selection.min.y = new_selection.max.y - new_h;
                    }
                    DragInteraction::Left => {
                        let max_w = selection.max.x - content_rect.left();
                        let max_h = (selection.center().y - content_rect.top())
                            .min(content_rect.bottom() - selection.center().y)
                            * 2.0;
                        let mut new_w = (selection.width() - mouse_delta.x).max(30.0);
                        let mut new_h = new_w / ratio;
                        if new_w > max_w {
                            new_w = max_w;
                            new_h = new_w / ratio;
                        }
                        if new_h > max_h {
                            new_h = max_h;
                            new_w = new_h * ratio;
                        }
                        let cy = selection.center().y;
                        new_selection.min.x = new_selection.max.x - new_w;
                        new_selection.min.y = cy - new_h / 2.0;
                        new_selection.max.y = cy + new_h / 2.0;
                    }
                    DragInteraction::Right => {
                        let max_w = content_rect.right() - selection.min.x;
                        let max_h = (selection.center().y - content_rect.top())
                            .min(content_rect.bottom() - selection.center().y)
                            * 2.0;
                        let mut new_w = (selection.width() + mouse_delta.x).max(30.0);
                        let mut new_h = new_w / ratio;
                        if new_w > max_w {
                            new_w = max_w;
                            new_h = new_w / ratio;
                        }
                        if new_h > max_h {
                            new_h = max_h;
                            new_w = new_h * ratio;
                        }
                        let cy = selection.center().y;
                        new_selection.max.x = new_selection.min.x + new_w;
                        new_selection.min.y = cy - new_h / 2.0;
                        new_selection.max.y = cy + new_h / 2.0;
                    }
                    DragInteraction::BottomLeft => {
                        let max_w = selection.max.x - content_rect.left();
                        let max_h = content_rect.bottom() - selection.min.y;
                        let mut new_w = (selection.width() - mouse_delta.x).max(30.0);
                        let mut new_h = (selection.height() + mouse_delta.y).max(20.0);
                        if new_w > new_h * ratio {
                            new_h = new_w / ratio;
                        } else {
                            new_w = new_h * ratio;
                        }
                        if new_w > max_w {
                            new_w = max_w;
                            new_h = new_w / ratio;
                        }
                        if new_h > max_h {
                            new_h = max_h;
                            new_w = new_h * ratio;
                        }
                        new_selection.min.x = new_selection.max.x - new_w;
                        new_selection.max.y = new_selection.min.y + new_h;
                    }
                    DragInteraction::Bottom => {
                        let max_w = (selection.center().x - content_rect.left())
                            .min(content_rect.right() - selection.center().x)
                            * 2.0;
                        let max_h = content_rect.bottom() - selection.min.y;
                        let mut new_h = (selection.height() + mouse_delta.y).max(20.0);
                        let mut new_w = new_h * ratio;
                        if new_w > max_w {
                            new_w = max_w;
                            new_h = new_w / ratio;
                        }
                        if new_h > max_h {
                            new_h = max_h;
                            new_w = new_h * ratio;
                        }
                        let cx = selection.center().x;
                        new_selection.max.y = new_selection.min.y + new_h;
                        new_selection.min.x = cx - new_w / 2.0;
                        new_selection.max.x = cx + new_w / 2.0;
                    }
                    DragInteraction::BottomRight => {
                        let max_w = content_rect.right() - selection.min.x;
                        let max_h = content_rect.bottom() - selection.min.y;
                        let mut new_w = (selection.width() + mouse_delta.x).max(30.0);
                        let mut new_h = (selection.height() + mouse_delta.y).max(20.0);
                        if new_w > new_h * ratio {
                            new_h = new_w / ratio;
                        } else {
                            new_w = new_h * ratio;
                        }
                        if new_w > max_w {
                            new_w = max_w;
                            new_h = new_w / ratio;
                        }
                        if new_h > max_h {
                            new_h = max_h;
                            new_w = new_h * ratio;
                        }
                        new_selection.max.x = new_selection.min.x + new_w;
                        new_selection.max.y = new_selection.min.y + new_h;
                    }
                }
                *selection = new_selection;
                if selection.min.x > selection.max.x {
                    std::mem::swap(&mut selection.min.x, &mut selection.max.x);
                    if let Some(d) = *dragging {
                        *dragging = Some(match d {
                            DragInteraction::TopLeft => DragInteraction::TopRight,
                            DragInteraction::Left => DragInteraction::Right,
                            DragInteraction::BottomLeft => DragInteraction::BottomRight,
                            DragInteraction::TopRight => DragInteraction::TopLeft,
                            DragInteraction::Right => DragInteraction::Left,
                            DragInteraction::BottomRight => DragInteraction::BottomLeft,
                            other => other,
                        });
                    }
                }
                if selection.min.y > selection.max.y {
                    std::mem::swap(&mut selection.min.y, &mut selection.max.y);
                    if let Some(d) = *dragging {
                        *dragging = Some(match d {
                            DragInteraction::TopLeft => DragInteraction::BottomLeft,
                            DragInteraction::Top => DragInteraction::Bottom,
                            DragInteraction::TopRight => DragInteraction::BottomRight,
                            DragInteraction::BottomLeft => DragInteraction::TopLeft,
                            DragInteraction::Bottom => DragInteraction::Top,
                            DragInteraction::BottomRight => DragInteraction::TopRight,
                            other => other,
                        });
                    }
                }
            }

            Area::new("selection_canvas".into())
                .fixed_pos(Pos2::ZERO)
                .show(ui, |ui| {
                    let screen_rect = ui.content_rect();
                    let (rect, _response) =
                        ui.allocate_exact_size(screen_rect.size(), Sense::click());
                    let painter = ui.painter();
                    let bg_color = Color32::from_rgba_unmultiplied(0, 0, 0, 96);
                    let s_rect = Rect::from_min_max(
                        selection.min.min(selection.max),
                        selection.min.max(selection.max),
                    );

                    painter.rect_filled(
                        Rect::from_min_max(rect.left_top(), pos2(rect.right(), s_rect.top())),
                        0.0,
                        bg_color,
                    );
                    painter.rect_filled(
                        Rect::from_min_max(pos2(rect.left(), s_rect.bottom()), rect.right_bottom()),
                        0.0,
                        bg_color,
                    );
                    painter.rect_filled(
                        Rect::from_min_max(
                            pos2(rect.left(), s_rect.top()),
                            pos2(s_rect.left(), s_rect.bottom()),
                        ),
                        0.0,
                        bg_color,
                    );
                    painter.rect_filled(
                        Rect::from_min_max(
                            pos2(s_rect.right(), s_rect.top()),
                            pos2(rect.right(), s_rect.bottom()),
                        ),
                        0.0,
                        bg_color,
                    );

                    let handle_radius = 5.0;
                    let handle_rect = |pos: Pos2| {
                        Rect::from_center_size(pos, vec2(handle_radius * 2.5, handle_radius * 2.5))
                    };
                    let handles = [
                        (DragInteraction::TopLeft, selection.left_top()),
                        (DragInteraction::Top, selection.center_top()),
                        (DragInteraction::TopRight, selection.right_top()),
                        (DragInteraction::Left, selection.left_center()),
                        (DragInteraction::Right, selection.right_center()),
                        (DragInteraction::BottomLeft, selection.left_bottom()),
                        (DragInteraction::Bottom, selection.center_bottom()),
                        (DragInteraction::BottomRight, selection.right_bottom()),
                    ];

                    for (interaction, pos) in &handles {
                        let handle_response = ui
                            .interact(
                                handle_rect(*pos),
                                ui.id().with(format!("{:?}", interaction)),
                                Sense::drag(),
                            )
                            .on_hover_cursor(interaction.cursor_icon());
                        if handle_response.drag_started() {
                            *dragging = Some(*interaction);
                        }
                    }

                    let body_rect = selection.shrink(handle_radius);
                    if body_rect.width() > 0.0 && body_rect.height() > 0.0 {
                        let body_response = ui
                            .interact(body_rect, ui.id().with("body"), Sense::drag())
                            .on_hover_cursor(DragInteraction::Body.cursor_icon());
                        if body_response.drag_started() {
                            *dragging = Some(DragInteraction::Body);
                        }
                    }
                    if ui.input(|i| i.pointer.any_released()) {
                        *dragging = None;
                    }

                    painter.rect_stroke(
                        *selection,
                        0.0,
                        Stroke::new(2.0_f32, Color32::WHITE),
                        StrokeKind::Outside,
                    );
                    painter.rect_stroke(
                        *selection,
                        0.0,
                        Stroke::new(1.0_f32, Color32::from_black_alpha(100)),
                        StrokeKind::Inside,
                    );
                    for (_, pos) in &handles {
                        painter.circle_filled(*pos, handle_radius, Color32::WHITE);
                        painter.circle_stroke(
                            *pos,
                            handle_radius,
                            Stroke::new(1.0_f32, Color32::from_black_alpha(128)),
                        );
                    }
                });

            let mut confirm = ui.input(|i| i.key_pressed(Key::Enter));
            let cancel = ui.input(|i| i.key_pressed(Key::Escape));

            if dragging.is_none() {
                let screen_rect = ui.content_rect();
                let mut window_pos = pos2(selection.center().x, selection.bottom() + 10.0);
                let pivot = if window_pos.y + 40.0 > screen_rect.max.y {
                    window_pos.y = selection.bottom() - 10.0;
                    Align2::CENTER_BOTTOM
                } else {
                    Align2::CENTER_TOP
                };

                let frame = Frame::popup(ui.style())
                    .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 245))
                    .stroke(Stroke::new(
                        1.0,
                        Color32::from_rgba_unmultiplied(0, 0, 0, 40),
                    ))
                    .corner_radius(20)
                    .inner_margin(Margin::symmetric(10, 6));
                Window::new("Selection Actions")
                    .frame(frame)
                    .pivot(pivot)
                    .fixed_pos(window_pos)
                    .collapsible(false)
                    .title_bar(false)
                    .resizable(false)
                    .order(egui::Order::Foreground)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let btn_size = egui::vec2(36.0, 36.0);

                            // OK button
                            let ok_resp = ui
                                .add_sized(
                                    btn_size,
                                    Button::new(
                                        RichText::new("✔")
                                            .size(18.0)
                                            .color(Color32::from_rgb(39, 174, 96)),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text("확인 (Enter)");

                            if ok_resp.hovered() {
                                ui.set_cursor_icon(egui::CursorIcon::PointingHand);
                                ui.painter().rect_filled(
                                    ok_resp.rect,
                                    8.0,
                                    Color32::from_rgba_unmultiplied(0, 0, 0, 25),
                                );
                            }
                            if ok_resp.clicked() {
                                confirm = true;
                            }

                            // Translucent Separator
                            ui.add_space(2.0);
                            let (separator_rect, _) =
                                ui.allocate_exact_size(egui::vec2(1.0, 20.0), egui::Sense::hover());
                            ui.painter().rect_filled(
                                separator_rect,
                                0.0,
                                Color32::from_rgba_unmultiplied(0, 0, 0, 35),
                            );
                            ui.add_space(2.0);

                            // Aspect Ratio Toggle button
                            let toggle_text = if self.portrait_mode { "▮" } else { "▭" };
                            let hover_text = if self.portrait_mode {
                                "가로 모드로 전환 (1.5:1)"
                            } else {
                                "세로 모드로 전환 (1:1.5)"
                            };
                            let toggle_color = Color32::from_rgb(9, 132, 227);
                            let toggle_resp = ui
                                .add_sized(
                                    btn_size,
                                    Button::new(
                                        RichText::new(toggle_text).size(18.0).color(toggle_color),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text(hover_text);

                            if toggle_resp.hovered() {
                                ui.set_cursor_icon(egui::CursorIcon::PointingHand);
                                ui.painter().rect_filled(
                                    toggle_resp.rect,
                                    8.0,
                                    Color32::from_rgba_unmultiplied(0, 0, 0, 25),
                                );
                            }
                            if toggle_resp.clicked() {
                                self.portrait_mode = !self.portrait_mode;
                                let center = selection.center();
                                let (new_w, new_h) = if self.portrait_mode {
                                    let h = selection.width().max(selection.height());
                                    (h / 1.5, h)
                                } else {
                                    let w = selection.width().max(selection.height());
                                    (w, w / 1.5)
                                };
                                *selection = Rect::from_center_size(center, vec2(new_w, new_h));
                            }
                        });
                    });
            }

            // 실시간으로 선택 영역을 백그라운드 capture에 반영
            if let Some(ref mut capture) = self.capture {
                let mut real_time_selection = *selection;
                if let (Some(window_rect), Some(monitor_pos)) = (
                    ui.input(|i| i.viewport().outer_rect),
                    capture_region.monitor_pos(),
                ) {
                    real_time_selection =
                        real_time_selection.translate(window_rect.min - monitor_pos);
                }
                let mut updated_region = capture_region.clone();
                updated_region.update_from_selection(real_time_selection);
                capture.region = Some(updated_region);
            }

            if confirm {
                if let (Some(window_rect), Some(monitor_pos)) = (
                    ui.input(|i| i.viewport().outer_rect),
                    capture_region.monitor_pos(),
                ) {
                    *selection = selection.translate(window_rect.min - monitor_pos);
                }
                capture_region.update_from_selection(*selection);
                if let Some(m_pos) = capture_region.monitor_pos() {
                    let logical_abs_x = m_pos.x + selection.min.x;
                    let logical_abs_y = m_pos.y + selection.min.y;
                    let logical_w = selection.width();
                    let logical_h = selection.height();
                    do_save_area = Some((
                        logical_abs_x.round() as i32,
                        logical_abs_y.round() as i32,
                        logical_w.round() as u32,
                        logical_h.round() as u32,
                    ));

                    let mut logical_region_height = logical_h;
                    if let Some(size) = capture_region.monitor_size()
                        && vec2(logical_w, logical_region_height) == size
                    {
                        logical_region_height -= 1.0;
                    }
                    let border_margin = 2.0;
                    ui.send_viewport_cmd(ViewportCommand::MousePassthrough(true));
                    ui.send_viewport_cmd(ViewportCommand::OuterPosition(pos2(
                        logical_abs_x - border_margin,
                        logical_abs_y - border_margin,
                    )));
                    ui.send_viewport_cmd(ViewportCommand::InnerSize(vec2(
                        logical_w + border_margin * 2.0,
                        logical_region_height + border_margin * 2.0,
                    )));
                }
                transition_to_capturing = Some(capture_region.clone());
            }
            if cancel {
                transition_to_initializing = true;
            }
        }

        if let Some(area) = do_save_area {
            self.saved_capture_area = Some(area);
            self.save_to_file();
        }
        if let Some(region) = transition_to_capturing {
            if self.capture.is_none()
                && let Some(app_comm) = &mut self.app_comm
            {
                let req = CaptureRequestToRuntime::LaunchApplet {
                    name: "mirror".to_string(),
                };
                if let Err(e) = app_comm.send_request(&req) {
                    log::error!("Failed to send LaunchApplet(mirror) request: {}", e);
                }
                if let Err(e) = app_comm.receive_response() {
                    log::error!("Failed to receive response: {}", e);
                }
            }
            self.transit_to_capturing(
                region,
                self.bits_per_pixel,
                self.saved_filter_mode,
                self.saved_invert_active,
                self.portrait_mode,
            );
        } else if transition_to_initializing {
            self.state.transit_to_initializing(None, None);
        }
    }

    fn ui_capturing(&mut self, ui: &mut egui::Ui) {
        let mut do_save = false;
        let mut transition_to_initializing = false;
        let mut transition_to_selecting = None;
        let mut launch_launcher = false;
        let mut new_invert_active = None;
        let mut new_filter_mode = None;

        if let State::Capturing = self.state
            && let Some(ref mut capture) = self.capture
        {
            let capture_state = &mut self.capture_state;
            if let Some(config_mutex) = &capture.config
                && let Ok(mut cfg) = config_mutex.lock()
            {
                cfg.region = capture.region.clone();
                cfg.filter_mode = capture.filter_mode;
                cfg.invert_active = capture.invert_active;
                cfg.capture_state = *capture_state;
                cfg.bits_per_pixel = self.bits_per_pixel;
                cfg.use_high_range = self.use_high_range;
            }
            let mut transition_to_selecting_state = None;
            let is_hovered = if let Some(outer_rect) = ui.input(|i| i.viewport().outer_rect) {
                let mouse = self.device_state.get_mouse();
                let sf = get_scale_factor(ui);
                let mouse_pos = egui::pos2(mouse.coords.0 as f32 / sf, mouse.coords.1 as f32 / sf);
                let margin = 6.0;
                let on_left_edge = (mouse_pos.x - outer_rect.left()).abs() <= margin
                    && mouse_pos.y >= outer_rect.top() - margin
                    && mouse_pos.y <= outer_rect.bottom() + margin;
                let on_right_edge = (mouse_pos.x - outer_rect.right()).abs() <= margin
                    && mouse_pos.y >= outer_rect.top() - margin
                    && mouse_pos.y <= outer_rect.bottom() + margin;
                let on_top_edge = (mouse_pos.y - outer_rect.top()).abs() <= margin
                    && mouse_pos.x >= outer_rect.left() - margin
                    && mouse_pos.x <= outer_rect.right() + margin;
                let on_bottom_edge = (mouse_pos.y - outer_rect.bottom()).abs() <= margin
                    && mouse_pos.x >= outer_rect.left() - margin
                    && mouse_pos.x <= outer_rect.right() + margin;
                let hovered = on_left_edge || on_right_edge || on_top_edge || on_bottom_edge;
                if hovered {
                    if let Some(start_time) = self.hover_start_time {
                        if start_time.elapsed().as_secs_f32() >= 1.0 {
                            if let Some(region) = &capture.region {
                                let sf = region.scale_factor;
                                let selection_rect = Rect::from_min_size(
                                    pos2(region.x as f32 / sf, region.y as f32 / sf),
                                    vec2(region.width as f32 / sf, region.height as f32 / sf),
                                );
                                let center_pos = selection_rect.center();
                                transition_to_selecting_state =
                                    Some((center_pos, region.clone(), Some(selection_rect)));
                            }
                            self.hover_start_time = None;
                        }
                    } else {
                        self.hover_start_time = Some(std::time::Instant::now());
                    }
                } else {
                    self.hover_start_time = None;
                }
                hovered
            } else {
                self.hover_start_time = None;
                false
            };

            if let Some(state_args) = transition_to_selecting_state {
                transition_to_selecting = Some(state_args);
            }

            egui::CentralPanel::default()
                .frame(Frame::new().fill(Color32::TRANSPARENT))
                .show_inside(ui, |ui| {
                    let rect = ui.viewport_rect();
                    let stroke_width = if is_hovered { 3.0 } else { 1.0 };
                    let stroke = Stroke::new(stroke_width, Color32::WHITE);
                    let rect = rect.shrink(stroke.width / 2.0);
                    let path = vec![
                        rect.left_top(),
                        rect.right_top(),
                        rect.right_bottom(),
                        rect.left_bottom(),
                        rect.left_top(),
                    ];
                    ui.painter().add(egui::Shape::line(
                        path.clone(),
                        Stroke::new(stroke.width, Color32::GRAY),
                    ));
                    ui.painter()
                        .extend(egui::Shape::dashed_line(&path, stroke, 4.0, 4.0));
                });
            ui.request_repaint();
        }

        let mut viewport_builder = egui::ViewportBuilder::default()
            .with_title("Capture Control")
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_inner_size(self.capture_min_window);
        if let State::Capturing = self.state
            && let Some(capture) = &self.capture
            && let Some(region) = &capture.region
            && let (Some(m_pos), Some(m_size)) = (region.monitor_pos(), region.monitor_size())
        {
            let initial_pos = pos2(
                m_pos.x + m_size.x / 2.0 - self.capture_min_window.x / 2.0,
                m_pos.y,
            );
            viewport_builder = viewport_builder.with_position(initial_pos);
        }

        ui.ctx().show_viewport_immediate(
            egui::ViewportId::from_hash_of("capture_control_window"),
            viewport_builder,
            |ui, _class| {
                if let State::Capturing = self.state
                    && let Some(ref mut capture) = self.capture
                    && !capture.window_pos_initialized {
                        if let Some(region) = &capture.region
                            && let (Some(mut m_pos), Some(mut m_size)) =
                                (region.monitor_pos(), region.monitor_size())
                    {
                        let mut capture_min_window_x = self.capture_min_window.x;
                        let scale_factor = get_scale_factor(ui);
                        m_pos.x /= scale_factor;
                        m_pos.y /= scale_factor;
                        m_size.x /= scale_factor;
                        m_size.y /= scale_factor;
                        capture_min_window_x /= scale_factor;
                        let initial_pos = pos2(
                            m_pos.x + m_size.x / 2.0 - capture_min_window_x / 2.0,
                            m_pos.y,
                        );
                        ui.send_viewport_cmd(ViewportCommand::OuterPosition(initial_pos));
                    }
                    capture.window_pos_initialized = true;
                }

                if let State::Capturing = self.state
                    && let Some(ref mut capture) = self.capture
                {
                    CentralPanel::default()
                        .frame(Frame::central_panel(ui.style()).inner_margin(0.0))
                        .show_inside(ui, |ui| {
                            let available_height = ui.available_height();
                            let preview_ratio = 1.5;
                            let image_width = available_height * preview_ratio;

                            Panel::left("capture_image_panel")
                                .frame(Frame::new().inner_margin(0.0))
                                .exact_size(image_width)
                                .resizable(false)
                                .show_inside(ui, |ui| {
                                    let available_size = ui.available_size();
                                    ui.centered_and_justified(|ui| {
                                        let container_aspect = vec2(3.0, 2.0);
                                        let container_size =
                                            fit_to_aspect_ratio(container_aspect, available_size);
                                        let (frame_rect, _) =
                                            ui.allocate_exact_size(container_size, Sense::hover());
                                        ui.painter().rect_stroke(
                                            frame_rect,
                                            0.0,
                                            Stroke::new(2.0_f32, Color32::from_gray(150)),
                                            StrokeKind::Inside,
                                        );
                                        let inner_rect = frame_rect.shrink(2.0);
                                        ui.scope_builder(
                                            egui::UiBuilder::new().max_rect(inner_rect),
                                            |ui| {
                                                ui.centered_and_justified(|ui| {
                                                    if let Some(pixels) = &capture.device_pixels {
                                                         let scaled_size = fit_to_aspect_ratio(
                                                             egui::vec2(DISPLAY_WIDTH as f32, DISPLAY_HEIGHT as f32),
                                                             inner_rect.size(),
                                                         );
                                                         let (rect, _response) = ui.allocate_exact_size(
                                                             scaled_size,
                                                             egui::Sense::hover(),
                                                         );
                                                         let painter = ui.painter();

                                                         // Draw background for the grid
                                                         painter.rect_filled(
                                                             rect,
                                                             6.0,
                                                             Color32::from_gray(15), // Elegant dark slate background
                                                         );

                                                         let step_x = scaled_size.x / DISPLAY_WIDTH as f32;
                                                         let step_y = scaled_size.y / DISPLAY_HEIGHT as f32;
                                                         let step_size = step_x.min(step_y);
                                                         let dot_radius = (step_size * 0.30).max(1.0); // Small, elegant braille dots

                                                         let active_color = Color32::from_gray(245); // High-contrast silver-white
                                                         let inactive_color = Color32::BLACK; // Black for inactive dots

                                                         for r in 0..DISPLAY_HEIGHT as usize {
                                                             for c in 0..DISPLAY_WIDTH as usize {
                                                                 let idx = r * DISPLAY_WIDTH as usize + c;
                                                                 let val = pixels.get(idx).copied().unwrap_or(0);

                                                                 let center_x = rect.min.x + (c as f32 + 0.5) * step_x;
                                                                 let center_y = rect.min.y + (r as f32 + 0.5) * step_y;
                                                                 let center = egui::pos2(center_x, center_y);

                                                                 let t = val as f32 / 255.0;

                                                                 // Interpolate dot color
                                                                 let r_val = (inactive_color.r() as f32 + (active_color.r() as f32 - inactive_color.r() as f32) * t) as u8;
                                                                 let g_val = (inactive_color.g() as f32 + (active_color.g() as f32 - inactive_color.g() as f32) * t) as u8;
                                                                 let b_val = (inactive_color.b() as f32 + (active_color.b() as f32 - inactive_color.b() as f32) * t) as u8;
                                                                 let color = Color32::from_rgb(r_val, g_val, b_val);

                                                                 let current_radius = dot_radius * (0.8 + 0.3 * t);

                                                                 painter.circle_filled(center, current_radius, color);
                                                             }
                                                         }
                                                    } else if let Some(texture) = capture.texture.as_ref() {
                                                        if texture.size() == [0, 0] {
                                                            ui.label(
                                                                RichText::new("Failed to capture screen.")
                                                                    .color(Color32::RED),
                                                            );
                                                        } else {
                                                            let image_size = egui::Vec2::new(
                                                                texture.size()[0] as f32,
                                                                texture.size()[1] as f32,
                                                            );
                                                            let scaled_size = fit_to_aspect_ratio(
                                                                image_size,
                                                                inner_rect.size(),
                                                            );
                                                            ui.add(
                                                                egui::Image::new(texture)
                                                                    .fit_to_exact_size(scaled_size),
                                                            );
                                                        }
                                                    } else {
                                                        ui.spinner();
                                                    }
                                                });
                                            },
                                        );
                                    });
                                });

                            egui::CentralPanel::default()
                                .frame(Frame::new())
                                .show_inside(ui, |ui| {
                                    ui.allocate_ui_with_layout(
                                        vec2(ui.available_width(), ui.available_height()),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.add_space(8.0);
                                    if ui.button("↩").clicked() {
                                        transition_to_initializing = true;
                                    }
                                    let mut selected_filter = capture.filter_mode;
                                    let mut changed = false;
                                    if ui.radio_value(&mut selected_filter, FilterMode::None, "None").changed() {
                                        changed = true;
                                    }
                                    if ui.radio_value(&mut selected_filter, FilterMode::HighContrast, "High Contrast").changed() {
                                        changed = true;
                                    }
                                    if ui.radio_value(&mut selected_filter, FilterMode::Outline, "Outline").changed() {
                                        changed = true;
                                    }
                                    if changed {
                                        capture.filter_mode = selected_filter;
                                        do_save = true;
                                        new_filter_mode = Some(capture.filter_mode);
                                    }
                                    let filter_active = capture.filter_mode != FilterMode::None;
                                    ui.add_space(4.0);
                                    if ui.checkbox(&mut capture.invert_active, "Invert").changed() {
                                        do_save = true;
                                        new_invert_active = Some(capture.invert_active);
                                    }
                                    ui.add_space(4.0);
                                    let slider_widget = egui::Slider::new(&mut capture.threshold, 1..=254).show_value(false);
                                    if ui.add_enabled(filter_active, slider_widget).changed() {
                                        self.saved_threshold = capture.threshold;
                                        do_save = true;
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.add_space(8.0);

                                            if ui
                                                .add(Button::new(
                                                    RichText::new("Quit"),
                                                ))
                                                .clicked()
                                            {
                                                launch_launcher = true;
                                                ui.send_viewport_cmd_to(
                                                    egui::ViewportId::ROOT,
                                                    ViewportCommand::Close,
                                                );
                                            }

                                            ui.add_space(8.0);

                                            let color = match &self.connection_status {
                                                ConnectionStatus::Connected { .. } => Color32::GREEN,
                                                ConnectionStatus::Disconnected => {
                                                    if self.app_comm.is_some() {
                                                        Color32::YELLOW
                                                    } else {
                                                        Color32::RED
                                                    }
                                                }
                                            };
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new("●").size(10.0).color(color),
                                                )
                                                .selectable(false),
                                            );

                                            if ui.button("Settings").clicked() {
                                                self.show_settings_window = true;
                                                ui.ctx().send_viewport_cmd_to(
                                                    egui::ViewportId::from_hash_of("settings_window"),
                                                    ViewportCommand::Focus,
                                                );
                                            }
                                        },
                                    );
                                },
                            );
                                });
                        });
                }

                if self.show_settings_window {
                    let mut is_open = self.show_settings_window;
                    let settings_window_size = if self.connection_status.is_connected()
                        && self.connection_type == ConnectionType::Serial
                    {
                        vec2(600.0, 480.0)
                    } else {
                        vec2(450.0, 280.0)
                    };

                    ui.ctx().show_viewport_immediate(
                        egui::ViewportId::from_hash_of("settings_window"),
                        egui::ViewportBuilder::default()
                            .with_title("Settings")
                            .with_inner_size(settings_window_size),
                        |ctx, _class| {
                            if ctx.input(|i| i.viewport().close_requested()) {
                                is_open = false;
                            }

                            Panel::bottom("settings_bottom").show_inside(ctx, |ui| {
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("Close").clicked() {
                                                is_open = false;
                                            }
                                        },
                                    );
                                });
                                ui.add_space(8.0);
                            });

                            egui::CentralPanel::default().show_inside(ctx, |ui| {
                                let is_active = self.app_comm.is_some();
                                ui.horizontal(|ui| {
                                    ui.add_enabled_ui(!is_active, |ui| {
                                        if ui
                                            .selectable_value(
                                                &mut self.connection_type,
                                                ConnectionType::Serial,
                                                "Serial",
                                            )
                                            .changed()
                                        {
                                            do_save = true;
                                        }
                                        if ui
                                            .selectable_value(
                                                &mut self.connection_type,
                                                ConnectionType::Network,
                                                "Network",
                                            )
                                            .changed()
                                        {
                                            do_save = true;
                                        }
                                        if ui
                                            .selectable_value(
                                                &mut self.connection_type,
                                                ConnectionType::WebSocket,
                                                "WebSocket",
                                            )
                                            .changed()
                                        {
                                            do_save = true;
                                        }
                                    });
                                });
                                ui.add_space(8.0);

                                match self.connection_type {
                                    ConnectionType::Serial => {
                                        let now = std::time::Instant::now();
                                        if self
                                            .last_port_refresh
                                            .is_none_or(|t| now.duration_since(t).as_secs() > 1)
                                        {
                                            self.available_serial_ports =
                                                host_utils::serial_comm::available_ports(Some((LINUX_CDC_ACM_VID, LINUX_CDC_ACM_PID)));
                                            self.last_port_refresh = Some(now);
                                        }

                                        egui::ScrollArea::vertical().max_height(120.0).show(
                                            ui,
                                            |ui| {
                                                if self.available_serial_ports.is_empty() {
                                                    ui.label("No serial ports found.");
                                                } else {
                                                    for port in &self.available_serial_ports {
                                                        if ui
                                                            .selectable_label(
                                                                self.serial_address == *port,
                                                                port,
                                                            )
                                                            .clicked()
                                                        {
                                                            self.serial_address = port.clone();
                                                            do_save = true;
                                                        }
                                                    }
                                                }
                                            },
                                        );
                                    }
                                    ConnectionType::Network => {
                                        ui.horizontal(|ui| {
                                            ui.label("Network Address:");
                                            if ui
                                                .add(
                                                    egui::TextEdit::singleline(
                                                        &mut self.network_address,
                                                    )
                                                    .hint_text("192.168.1.100 (port 3006)")
                                                    .desired_width(180.0),
                                                )
                                                .changed()
                                            {
                                                do_save = true;
                                            }
                                        });
                                    }
                                    ConnectionType::WebSocket => {
                                        ui.horizontal(|ui| {
                                            ui.label("WebSocket Address:");
                                            if ui
                                                .add(
                                                    egui::TextEdit::singleline(
                                                        &mut self.websocket_address,
                                                    )
                                                    .hint_text("0.0.0.0:3007 (Server)")
                                                    .desired_width(180.0),
                                                )
                                                .changed()
                                            {
                                                do_save = true;
                                            }
                                        });
                                    }
                                }

                                ui.add_space(8.0);

                                let is_fully_connected =
                                    self.connection_status.is_connected();

                                ui.horizontal(|ui| {
                                    let button_text = if is_active {
                                        if is_fully_connected {
                                            "Disconnect"
                                        } else {
                                            "Cancel"
                                        }
                                    } else {
                                        "Connect"
                                    };

                                    let action_triggered = ui.button(button_text).clicked();

                                    if action_triggered {
                                        do_save = true;
                                        if is_active {
                                            if is_fully_connected
                                                && let Some(app_comm) = &mut self.app_comm
                                            {
                                                let req =
                                                    CaptureRequestToRuntime::LaunchApplet {
                                                        name: "launcher".to_string(),
                                                    };
                                                let _ = app_comm.send_request(&req);
                                                if let Err(e) = app_comm.receive_response()
                                                {
                                                    log::error!(
                                                        "Failed to receive response: {}",
                                                        e
                                                    );
                                                }
                                            }
                                            self.app_comm = None;
                                        } else {
                                            match self.connection_type {
                                                ConnectionType::Serial => {
                                                    match UsbComm::new(
                                                        &self.serial_address,
                                                        self.baud_rate,
                                                    ) {
                                                        Ok(comm) => {
                                                            self.app_comm =
                                                                Some(AppComm::Usb(comm))
                                                        }
                                                        Err(e) => log::error!(
                                                            "Failed to initialize USB comm: {}",
                                                            e
                                                        ),
                                                    }
                                                }
                                                ConnectionType::Network => {
                                                    let mut addr =
                                                        self.network_address.trim().to_string();
                                                    if !addr.is_empty() && !addr.contains(':') {
                                                        addr = format!("{}:3006", addr);
                                                    }
                                                    match NetComm::new(&addr) {
                                                        Ok(comm) => {
                                                            self.app_comm =
                                                                Some(AppComm::Net(comm))
                                                        }
                                                        Err(e) => log::error!(
                                                            "Failed to initialize Net comm: {}",
                                                            e
                                                        ),
                                                    }
                                                }
                                                ConnectionType::WebSocket => {
                                                    let mut addr = self
                                                        .websocket_address
                                                        .trim()
                                                        .to_string();
                                                    if !addr.is_empty() && !addr.contains(':') {
                                                        addr = format!("{}:3007", addr);
                                                    }
                                                    match WsComm::new(&addr) {
                                                        Ok(comm) => {
                                                            self.app_comm =
                                                                Some(AppComm::Ws(comm))
                                                        }
                                                        Err(e) => log::error!(
                                                            "Failed to initialize WS comm: {}",
                                                            e
                                                        ),
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    ui.add_space(4.0);

                                    if is_active {
                                        if !is_fully_connected {
                                            ui.add(egui::Spinner::new());
                                            ui.add_space(4.0);

                                            let status_text = match self.connection_type {
                                                ConnectionType::Serial => format!(
                                                    "Connecting to Serial: {}...",
                                                    self.serial_address
                                                ),
                                                ConnectionType::Network => format!(
                                                    "Connecting to Network: {}...",
                                                    self.network_address
                                                ),
                                                ConnectionType::WebSocket => format!(
                                                    "Connecting to WebSocket: {}...",
                                                    self.websocket_address
                                                ),
                                            };
                                            ui.colored_label(
                                                egui::Color32::from_rgb(255, 165, 0),
                                                status_text,
                                            );
                                        } else {
                                            let status_text = match self.connection_type {
                                                ConnectionType::Serial => format!(
                                                    "Connected to Serial: {}",
                                                    self.serial_address
                                                ),
                                                ConnectionType::Network => format!(
                                                    "Connected to Network: {}",
                                                    self.network_address
                                                ),
                                                ConnectionType::WebSocket => format!(
                                                    "Connected to WebSocket: {}",
                                                    self.websocket_address
                                                ),
                                            };
                                            ui.colored_label(egui::Color32::GREEN, status_text);
                                        }
                                    } else {
                                        ui.weak("Disconnected");
                                    }
                                });

                                if self.connection_status.is_connected()
                                    && self.connection_type == ConnectionType::Serial
                                {
                                    ui.add_space(8.0);
                                    ui.separator();
                                    ui.add_space(8.0);
                                    ui.heading("Wi-Fi Settings");
                                    ui.add_space(4.0);

                                    egui::ScrollArea::vertical()
                                        .max_height(200.0)
                                        .id_salt("wifi_settings_scroll")
                                        .show(ui, |ui| {
                                            ui.columns(2, |columns| {
                                                // Left column (Discovery)
                                                let ui_left = &mut columns[0];

                                                ui_left.horizontal(|ui| {
                                                    let is_busy = self.wifi_scanning || self.wifi_connecting;
                                                    if ui.add_enabled(!is_busy, egui::Button::new("Scan Networks")).clicked() {
                                                        self.wifi_scanning = true;
                                                        self.wifi_ssids.clear();
                                                        if let Some(comm) = &mut self.app_comm {
                                                            let _ = comm.send_request(&CaptureRequestToRuntime::ScanWifi);
                                                        }
                                                    }
                                                    if self.wifi_scanning {
                                                        ui.add(egui::Spinner::new());
                                                        ui.label("Scanning...");
                                                    }
                                                });
                                                ui_left.add_space(4.0);

                                                if !self.wifi_ssids.is_empty() {
                                                    ui_left.label("Scanned Networks:");
                                                    egui::ScrollArea::vertical()
                                                        .max_height(100.0)
                                                        .id_salt("wifi_networks_scroll")
                                                        .show(ui_left, |ui| {
                                                            for net in &self.wifi_ssids {
                                                                let label = format!("{} ({}%) - {}", net.ssid, net.signal, net.security);
                                                                if ui.selectable_label(self.selected_ssid == net.ssid, label).clicked() {
                                                                    self.selected_ssid = net.ssid.clone();
                                                                }
                                                            }
                                                        });
                                                } else if !self.wifi_scanning {
                                                    ui_left.weak("No networks scanned. Click 'Scan Networks' to search.");
                                                }

                                                // Right column (Configuration & Control)
                                                let ui_right = &mut columns[1];

                                                if !self.wifi_ips.is_empty() {
                                                    ui_right.horizontal(|ui| {
                                                        ui.strong("IP Addresses:");
                                                        ui.label(self.wifi_ips.join(", "));
                                                    });
                                                    ui_right.add_space(4.0);
                                                }

                                                egui::Grid::new("wifi_inputs_grid")
                                                    .num_columns(2)
                                                    .spacing([8.0, 6.0])
                                                    .show(ui_right, |ui| {
                                                        ui.label("SSID:");
                                                        ui.add_enabled(false, egui::TextEdit::singleline(&mut self.selected_ssid).desired_width(140.0));
                                                        ui.end_row();

                                                        ui.label("Passphrase:");
                                                        ui.add(egui::TextEdit::singleline(&mut self.wifi_passphrase).password(true).desired_width(140.0));
                                                        ui.end_row();
                                                    });
                                                ui_right.add_space(8.0);

                                                ui_right.horizontal(|ui| {
                                                    let is_busy = self.wifi_scanning || self.wifi_connecting;
                                                    let can_connect = !self.selected_ssid.is_empty() && !is_busy;

                                                    let is_connected_now = matches!(&self.wifi_connect_result, Some(Ok(_)));

                                                    if is_connected_now {
                                                        if ui.add_enabled(!is_busy, egui::Button::new("Disconnect")).clicked() {
                                                            self.wifi_connecting = true;
                                                            self.wifi_disconnect_result = None;
                                                            self.wifi_ips.clear();
                                                            if let Some(comm) = &mut self.app_comm {
                                                                let _ = comm.send_request(&CaptureRequestToRuntime::DisconnectWifi {
                                                                    ssid: self.selected_ssid.clone(),
                                                                });
                                                            }
                                                        }
                                                    } else {
                                                        if ui.add_enabled(can_connect, egui::Button::new("Connect")).clicked() {
                                                            self.wifi_connecting = true;
                                                            self.wifi_connect_result = None;
                                                            self.wifi_disconnect_result = None;
                                                            self.wifi_ips.clear();
                                                            if let Some(comm) = &mut self.app_comm {
                                                                let _ = comm.send_request(&CaptureRequestToRuntime::ConnectWifi {
                                                                    ssid: self.selected_ssid.clone(),
                                                                    passphrase: self.wifi_passphrase.clone(),
                                                                });
                                                            }
                                                        }
                                                    }

                                                    if self.wifi_connecting {
                                                        ui.add(egui::Spinner::new());
                                                        if is_connected_now {
                                                            ui.label("Disconnecting...");
                                                        } else {
                                                            ui.label("Connecting...");
                                                        }
                                                    }
                                                });

                                                if let Some(res) = &self.wifi_connect_result {
                                                    ui_right.add_space(4.0);
                                                    match res {
                                                        Ok(_msg) => {
                                                            ui_right.colored_label(egui::Color32::GREEN, "Success".to_string());
                                                        }
                                                        Err(err) => {
                                                            ui_right.colored_label(egui::Color32::RED, format!("Error: {}", err));
                                                        }
                                                    }
                                                } else if let Some(res) = &self.wifi_disconnect_result {
                                                    ui_right.add_space(4.0);
                                                    match res {
                                                        Ok(msg) => {
                                                            ui_right.weak(msg);
                                                        }
                                                        Err(err) => {
                                                            ui_right.colored_label(egui::Color32::RED, format!("Error: {}", err));
                                                        }
                                                    }
                                                }
                                            });
                                        });
                                }
                            });
                        },
                    );

                    self.show_settings_window = is_open;
                }

                if ui.input(|i| i.pointer.any_pressed()) && !ui.egui_wants_pointer_input() {
                    ui.send_viewport_cmd(ViewportCommand::StartDrag);
                }
            },
        );

        if let Some(v) = new_invert_active {
            self.saved_invert_active = v;
        }
        if let Some(v) = new_filter_mode {
            self.saved_filter_mode = v;
        }
        if launch_launcher && let Some(app_comm) = &mut self.app_comm {
            let req = CaptureRequestToRuntime::LaunchApplet {
                name: "launcher".to_string(),
            };
            let _ = app_comm.send_request(&req);
            if let Err(e) = app_comm.receive_response() {
                log::error!("Failed to receive response: {}", e);
            }
        }
        if do_save {
            self.save_to_file();
        }
        if let Some((center_pos, region, saved_selection)) = transition_to_selecting {
            if let Some(ref mut capture) = self.capture {
                capture.texture = None;
            }
            select_window_setup(ui, &region);
            self.state
                .transit_to_pre_selecting(center_pos, region, saved_selection);
        } else if transition_to_initializing {
            if let Some(capture) = &self.capture
                && let Some(capture_region) = capture.region.clone()
            {
                let sf = capture_region.scale_factor;
                let logical_x = capture_region.x as f32 / sf;
                let logical_y = capture_region.y as f32 / sf;
                let logical_w = capture_region.width as f32 / sf;
                let logical_h = capture_region.height as f32 / sf;
                let center_pos = if let Some(m_pos) = capture_region.monitor_pos() {
                    pos2(
                        m_pos.x + logical_x + logical_w / 2.0,
                        m_pos.y + logical_y + logical_h / 2.0,
                    )
                } else {
                    pos2(logical_x + logical_w / 2.0, logical_y + logical_h / 2.0)
                };
                self.state
                    .transit_to_initializing(Some(center_pos), Some(capture_region));
            } else {
                self.state.transit_to_initializing(None, None);
            }
        }
    }
}

impl Default for CaptureApp {
    fn default() -> Self {
        Self {
            state: State::default(),
            capture_min_window: Vec2::ZERO,
            app_comm: None,
            bits_per_pixel: 4,
            connection_status: ConnectionStatus::Disconnected,
            connection_type: ConnectionType::Serial,
            serial_address: String::new(),
            network_address: String::new(),
            websocket_address: String::new(),
            baud_rate: 460800,
            saved_filter_mode: FilterMode::HighContrast,
            saved_invert_active: false,
            saved_capture_area: None,
            saved_threshold: 128,
            available_serial_ports: Vec::new(),
            last_port_refresh: None,
            show_settings_window: false,
            wifi_ssids: Vec::new(),
            wifi_scanning: false,
            selected_ssid: String::new(),
            wifi_passphrase: String::new(),
            wifi_connecting: false,
            wifi_connect_result: None,
            wifi_disconnect_result: None,
            wifi_ips: Vec::new(),
            device_state: DeviceState::new(),
            hover_start_time: None,
            portrait_mode: false,
            capture: None,
            capture_state: false,
            use_high_range: false,
        }
    }
}

impl eframe::App for CaptureApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.update_capture(ui.ctx());
        if let Some(app_comm) = &mut self.app_comm {
            while let Some(status) = app_comm.try_receive_status() {
                if status.is_connected() && !self.connection_status.is_connected() {
                    // Serial connected! Query network addresses immediately (could have ethernet, etc.)
                    let _ = app_comm.send_request(
                        &protocols::capture::CaptureRequestToRuntime::GetNetworkAddresses,
                    );

                    // Also trigger Wi-Fi network scanning immediately on connection
                    self.wifi_scanning = true;
                    self.wifi_ssids.clear();
                    let _ = app_comm
                        .send_request(&protocols::capture::CaptureRequestToRuntime::ScanWifi);

                    if self.capture.is_some() {
                        let req = CaptureRequestToRuntime::LaunchApplet {
                            name: "mirror".to_string(),
                        };
                        if let Err(e) = app_comm.send_request(&req) {
                            log::error!("Failed to send LaunchApplet(mirror) request: {}", e);
                        }
                        if let Err(e) = app_comm.receive_response() {
                            log::error!("Failed to receive response: {}", e);
                        }
                    }
                }
                if let ConnectionStatus::Connected {
                    port: Some(ref port),
                } = status
                {
                    self.serial_address = port.clone();
                }
                self.connection_status = status;
            }
            while let Some(resp_res) = app_comm.try_receive_response() {
                match resp_res {
                    Ok(protocols::capture::CaptureResponseFromRuntime::WifiScanResult(
                        networks,
                    )) => {
                        self.wifi_ssids = networks;
                        self.wifi_scanning = false;
                    }
                    Ok(protocols::capture::CaptureResponseFromRuntime::WifiConnectResult {
                        success,
                        message,
                    }) => {
                        self.wifi_connecting = false;
                        self.wifi_disconnect_result = None;
                        if success {
                            self.wifi_connect_result = Some(Ok(message));
                        } else {
                            self.wifi_connect_result = Some(Err(message));
                        }
                        // Query network addresses to update IP display (both on connection success or disconnection)
                        let _ = app_comm.send_request(
                            &protocols::capture::CaptureRequestToRuntime::GetNetworkAddresses,
                        );
                    }
                    Ok(protocols::capture::CaptureResponseFromRuntime::WifiDisconnectResult {
                        success,
                        message,
                    }) => {
                        self.wifi_connecting = false;
                        if success {
                            self.wifi_connect_result = None;
                            self.wifi_disconnect_result = Some(Ok(message));
                        } else {
                            self.wifi_disconnect_result = Some(Err(message));
                        }
                        let _ = app_comm.send_request(
                            &protocols::capture::CaptureRequestToRuntime::GetNetworkAddresses,
                        );
                    }
                    Ok(protocols::capture::CaptureResponseFromRuntime::NetworkAddressesResult(
                        addresses,
                    )) => {
                        self.wifi_ips = addresses;
                    }
                    Ok(protocols::capture::CaptureResponseFromRuntime::Paused) => {
                        self.capture_state = false;
                    }
                    Err(e) => {
                        log::error!("Comm error: {}", e);
                        if e.kind() == std::io::ErrorKind::BrokenPipe {
                            self.app_comm = None;
                            break;
                        }
                    }
                    _ => {}
                }
            }
        } else {
            self.connection_status = ConnectionStatus::Disconnected;
        }

        // Reset Wi-Fi configuration/scanning state if serial connection is lost or not active
        if !(self.connection_status.is_connected()
            && self.connection_type == ConnectionType::Serial)
        {
            self.wifi_scanning = false;
            self.wifi_connecting = false;
            self.wifi_connect_result = None;
            self.wifi_disconnect_result = None;
            self.wifi_ips.clear();
            self.wifi_ssids.clear();
            self.selected_ssid.clear();
            self.wifi_passphrase.clear();
        }

        match self.state {
            State::Initializing { .. } => self.ui_initializing(ui),
            State::PreSelecting { .. } => self.ui_pre_selecting(ui),
            State::Selecting { .. } => self.ui_selecting(ui),
            State::Capturing => self.ui_capturing(ui),
        };
    }

    fn clear_color(&self, _visuals: &Visuals) -> [f32; 4] {
        Color32::TRANSPARENT.to_normalized_gamma_f32()
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("serial_address", self.serial_address.clone());
        storage.set_string("network_address", self.network_address.clone());
        storage.set_string("websocket_address", self.websocket_address.clone());
        storage.set_string(
            "connection_type",
            match self.connection_type {
                ConnectionType::Serial => "Serial".to_string(),
                ConnectionType::Network => "Network".to_string(),
                ConnectionType::WebSocket => "WebSocket".to_string(),
            },
        );
        storage.set_string(
            "filter_mode",
            match self.saved_filter_mode {
                FilterMode::None => "None",
                FilterMode::HighContrast => "HighContrast",
                FilterMode::Outline => "Outline",
            }
            .to_string(),
        );
        storage.set_string("invert_active", self.saved_invert_active.to_string());
        if let Some((x, y, w, h)) = self.saved_capture_area {
            storage.set_string("capture_x", x.to_string());
            storage.set_string("capture_y", y.to_string());
            storage.set_string("capture_w", w.to_string());
            storage.set_string("capture_h", h.to_string());
        }
        self.save_to_file();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(app_comm) = &mut self.app_comm {
            let req = CaptureRequestToRuntime::LaunchApplet {
                name: "launcher".to_string(),
            };
            let _ = app_comm.send_request(&req);
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }
}
