#![windows_subsystem = "windows"]

mod app;
mod capture;
mod comm;
mod net_comm;
mod settings;
mod ui;
mod usb_comm;
mod ws_comm;

// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
// #![allow(rustdoc::missing_crate_level_docs)] // it's an example
use clap::Parser;
use eframe::egui::{Context, vec2};
use host_utils::serial_comm::ConnectionStatus;

use crate::app::{CaptureApp, LINUX_CDC_ACM_PID, LINUX_CDC_ACM_VID};
use crate::capture::FilterMode;
use crate::comm::{AppComm, ConnectionType};
use crate::net_comm::NetComm;
use crate::settings::{CaptureArea, CaptureSettings, get_settings_path};
use crate::usb_comm::UsbComm;
use crate::ws_comm::WsComm;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// 사용할 시리얼 포트 이름 (Windows 예: "COM3", Linux/macOS 예: "/dev/ttyUSB0" 등)
    #[arg(short, long)]
    port: Option<String>,

    /// 통신 속도 (Baud rate)
    #[arg(short, long, default_value_t = 460800)]
    baud_rate: u32,

    /// 픽셀 당 비트 수 (1, 2, 4, 8)
    #[arg(long, default_value_t = 4)]
    bits_per_pixel: u8,
}

fn main() -> eframe::Result {
    tracing_subscriber::fmt::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let args = Args::parse();

    let mut app_comm = None;
    if let Some(port_name) = &args.port {
        match UsbComm::new(port_name, args.baud_rate) {
            Ok(comm) => app_comm = Some(AppComm::Usb(comm)),
            Err(e) => tracing::error!("Failed to initialize USB comm: {}", e),
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false) // Hide the OS-specific "chrome" around the window
            .with_transparent(true) // To have rounded corners we need transparency
            .with_always_on_top(),
        ..Default::default()
    };
    eframe::run_native(
        "moredream_capture", // Used for window title and storage app_id
        options,
        Box::new(move |cc| {
            setup_custom_fonts(&cc.egui_ctx);

            // 글로벌 스타일 지정 (전체적인 여백과 모서리 둥글기 부여)
            cc.egui_ctx.all_styles_mut(|style| {
                // style.visuals.window_corner_radius = 8.into();
                // style.visuals.widgets.inactive.corner_radius = 3.0.into();
                // style.visuals.widgets.hovered.corner_radius = 3.0.into();
                // style.visuals.widgets.active.corner_radius = 3.0.into();
                style.spacing.button_padding = vec2(8.0, 6.0);
                // style.spacing.item_spacing = vec2(8.0, 8.0);
            });

            let mut serial_address = args.port.clone().unwrap_or_default();
            let mut network_address = String::new();
            let mut websocket_address = String::new();
            let mut connection_type = ConnectionType::Serial;
            let mut saved_filter_mode = FilterMode::HighContrast;
            let mut saved_invert_active = false;
            let mut saved_capture_area = None;
            let mut saved_threshold = 128u8;
            let mut portrait_mode = false;
            let mut use_high_range = false;

            // 즉각적인 파일 저장 로직을 추가했으므로 파일에서 설정을 우선적으로 불러옵니다.
            if let Ok(data) = std::fs::read_to_string(get_settings_path()) {
                if let Ok(settings) = toml::from_str::<CaptureSettings>(&data) {
                    if args.port.is_none() {
                        serial_address = settings.serial_address;
                    }
                    network_address = settings.network_address;
                    websocket_address = settings.websocket_address;
                    connection_type = if settings.connection_type == "Network" {
                        ConnectionType::Network
                    } else if settings.connection_type == "WebSocket" {
                        ConnectionType::WebSocket
                    } else {
                        ConnectionType::Serial
                    };
                    saved_filter_mode = match settings.filter_mode.as_str() {
                        "None" => FilterMode::None,
                        "Outline" => FilterMode::Outline,
                        _ => FilterMode::HighContrast,
                    };
                    saved_invert_active = settings.invert_active;
                    if let Some(area) = settings.capture_area {
                        saved_capture_area = Some((area.x, area.y, area.w, area.h));
                    }
                    saved_threshold = settings.threshold;
                    portrait_mode = settings.portrait_mode;
                    use_high_range = settings.use_high_range;
                }
            } else if let Some(storage) = cc.storage {
                // 파일이 없을 경우 기존의 eframe Storage를 대체용으로 사용합니다.
                if args.port.is_none()
                    && let Some(saved_serial) = storage.get_string("serial_address")
                {
                    serial_address = saved_serial;
                }
                if let Some(saved_network) = storage.get_string("network_address") {
                    network_address = saved_network;
                }
                if let Some(saved_ws) = storage.get_string("websocket_address") {
                    websocket_address = saved_ws;
                }
                if let Some(saved_type) = storage.get_string("connection_type") {
                    connection_type = if saved_type == "Network" {
                        ConnectionType::Network
                    } else if saved_type == "WebSocket" {
                        ConnectionType::WebSocket
                    } else {
                        ConnectionType::Serial
                    };
                }
                if let Some(mode) = storage.get_string("filter_mode") {
                    saved_filter_mode = match mode.as_str() {
                        "HighContrast" | "Detail" => FilterMode::HighContrast,
                        "Outline" => FilterMode::Outline,
                        _ => FilterMode::None,
                    };
                }
                if let Some(s) = storage.get_string("invert_active") {
                    saved_invert_active = s == "true";
                }
                if let Some(s) = storage.get_string("portrait_mode") {
                    portrait_mode = s == "true";
                }
                if let Some(s) = storage.get_string("use_high_range") {
                    use_high_range = s == "true";
                }
                if let (Some(x), Some(y), Some(w), Some(h)) = (
                    storage.get_string("capture_x"),
                    storage.get_string("capture_y"),
                    storage.get_string("capture_w"),
                    storage.get_string("capture_h"),
                ) && let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
                    x.parse::<i32>(),
                    y.parse::<i32>(),
                    w.parse::<u32>(),
                    h.parse::<u32>(),
                ) {
                    saved_capture_area = Some((x, y, w, h));
                }
            }

            if args.port.is_none() {
                let ports = host_utils::serial_comm::available_ports(Some((
                    LINUX_CDC_ACM_VID,
                    LINUX_CDC_ACM_PID,
                )));
                let mut settings_changed = false;
                if !serial_address.is_empty() && !ports.contains(&serial_address) {
                    serial_address = String::new();
                    settings_changed = true;
                    if ports.len() == 1 {
                        serial_address = ports[0].clone();
                    }
                } else if serial_address.is_empty() && ports.len() == 1 {
                    serial_address = ports[0].clone();
                    settings_changed = true;
                }

                if settings_changed {
                    let conn_type_str = match connection_type {
                        ConnectionType::Network => "Network",
                        ConnectionType::WebSocket => "WebSocket",
                        _ => "Serial",
                    };
                    let filter_mode_str = match saved_filter_mode {
                        FilterMode::None => "None",
                        FilterMode::Outline => "Outline",
                        _ => "HighContrast",
                    };
                    let capture_area_opt =
                        saved_capture_area.map(|(x, y, w, h)| CaptureArea { x, y, w, h });
                    let updated_settings = CaptureSettings {
                        serial_address: serial_address.clone(),
                        network_address: network_address.clone(),
                        websocket_address: websocket_address.clone(),
                        connection_type: conn_type_str.to_string(),
                        filter_mode: filter_mode_str.to_string(),
                        invert_active: saved_invert_active,
                        capture_area: capture_area_opt,
                        threshold: saved_threshold,
                        portrait_mode,
                        use_high_range,
                    };
                    if let Ok(data) = toml::to_string_pretty(&updated_settings) {
                        let _ = std::fs::write(get_settings_path(), data);
                    }
                }
            }

            let mut final_app_comm = app_comm;

            // CLI로 포트가 지정되지 않아서 연결되지 않은 상태라면, 저장된 설정으로 자동 연결을 시도합니다.
            if final_app_comm.is_none() {
                match connection_type {
                    ConnectionType::Serial => {
                        if !serial_address.is_empty() {
                            match UsbComm::new(&serial_address, args.baud_rate) {
                                Ok(comm) => final_app_comm = Some(AppComm::Usb(comm)),
                                Err(e) => tracing::error!("Failed to auto-connect USB comm: {}", e),
                            }
                        }
                    }
                    ConnectionType::Network => {
                        if !network_address.is_empty() {
                            let mut addr = network_address.trim().to_string();
                            if !addr.is_empty() && !addr.contains(':') {
                                addr = format!("{}:3006", addr);
                            }
                            match NetComm::new(&addr) {
                                Ok(comm) => final_app_comm = Some(AppComm::Net(comm)),
                                Err(e) => tracing::error!("Failed to auto-connect Net comm: {}", e),
                            }
                        }
                    }
                    ConnectionType::WebSocket => {
                        if !websocket_address.is_empty() {
                            let mut addr = websocket_address.trim().to_string();
                            if !addr.is_empty() && !addr.contains(':') {
                                addr = format!("{}:3007", addr);
                            }
                            match WsComm::new(&addr) {
                                Ok(comm) => final_app_comm = Some(AppComm::Ws(comm)),
                                Err(e) => tracing::error!("Failed to auto-connect WS comm: {}", e),
                            }
                        }
                    }
                }
            }

            Ok(Box::new(CaptureApp {
                app_comm: final_app_comm,
                bits_per_pixel: args.bits_per_pixel,
                connection_status: ConnectionStatus::Disconnected,
                connection_type,
                serial_address: serial_address.to_string(),
                network_address,
                websocket_address,
                baud_rate: args.baud_rate,
                saved_filter_mode,
                saved_invert_active,
                saved_capture_area,
                saved_threshold,
                portrait_mode,
                use_high_range,
                ..Default::default()
            }))
        }),
    )
}

fn setup_custom_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "nanum_gothic".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/NanumGothic.ttf")).into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "nanum_gothic".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("nanum_gothic".to_owned());
    ctx.set_fonts(fonts);
}
