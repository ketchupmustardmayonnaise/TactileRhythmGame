use eframe::egui;
use egui::{Pos2, Rect, Vec2, pos2, vec2};
use image::GrayImage;
use imageproc::{
    contrast::{ThresholdType, threshold_mut},
    distance_transform::Norm,
    filter::median_filter,
    morphology::dilate_mut,
};
use std::sync::{Arc, Mutex, mpsc};
use xcap::Monitor;

use crate::ui::get_scale_factor;

#[derive(Clone)]
pub struct CaptureRegion {
    pub monitor_id: Option<u32>,
    pub monitor_pos: Option<Pos2>,
    pub monitor_size: Option<Vec2>,
    pub scale_factor: f32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Default for CaptureRegion {
    fn default() -> Self {
        Self {
            monitor_id: None,
            monitor_pos: None,
            monitor_size: None,
            scale_factor: 1.0,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }
}

impl CaptureRegion {
    pub fn select_monitor(&mut self, ui: &egui::Ui, cx: i32, cy: i32) {
        let sf = get_scale_factor(ui);
        let px = (cx as f32 * sf).round() as i32;
        let py = (cy as f32 * sf).round() as i32;

        if let Ok(monitors) = Monitor::all() {
            let mut found_monitor = None;

            if let Ok(m) = Monitor::from_point(px, py) {
                found_monitor = Some(m);
            }

            if found_monitor.is_none()
                && let Some(logical_size) = ui.input(|i| i.viewport().monitor_size)
            {
                let expected_w = logical_size.x * sf;
                let expected_h = logical_size.y * sf;

                for m in &monitors {
                    if let (Ok(w), Ok(h)) = (m.width(), m.height()) {
                        let w_diff = (w as f32 - expected_w).abs();
                        let h_diff = (h as f32 - expected_h).abs();
                        if w_diff < 5.0 && h_diff < 5.0 {
                            found_monitor = Some(m.clone());
                            break;
                        }
                    }
                }
            }

            if let Some(monitor) = found_monitor.or_else(|| monitors.first().cloned()) {
                self.monitor_id = monitor.id().ok();
                self.scale_factor = sf;

                if let (Ok(x), Ok(y)) = (monitor.x(), monitor.y()) {
                    self.monitor_pos = Some(pos2(x as f32 / sf, y as f32 / sf));
                }
                if let (Ok(w), Ok(h)) = (monitor.width(), monitor.height()) {
                    self.monitor_size = Some(vec2(w as f32 / sf, h as f32 / sf));
                }
            }
        }
    }

    pub fn update_from_selection(&mut self, selection: Rect) {
        match &self.monitor_id {
            Some(_) => {
                let s_rect = Rect::from_min_max(
                    selection.min.min(selection.max),
                    selection.min.max(selection.max),
                );
                let sf = self.scale_factor;
                self.x = (s_rect.min.x * sf).round() as u32;
                self.y = (s_rect.min.y * sf).round() as u32;
                self.width = (s_rect.width() * sf).round() as u32;
                self.height = (s_rect.height() * sf).round() as u32;
            }
            None => {
                log::warn!("모니터 선택안됨");
            }
        }
    }

    pub fn monitor_contains(&self, cx: i32, cy: i32) -> bool {
        if let (Some(pos), Some(size)) = (self.monitor_pos, self.monitor_size) {
            let rect = Rect::from_min_size(pos, size);
            rect.contains(pos2(cx as f32, cy as f32))
        } else {
            false
        }
    }

    pub fn monitor_pos(&self) -> Option<Pos2> {
        self.monitor_pos
    }
    pub fn monitor_size(&self) -> Option<Vec2> {
        self.monitor_size
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterMode {
    None,
    HighContrast,
    Outline,
}

#[derive(Clone)]
pub struct CaptureConfig {
    pub region: Option<CaptureRegion>,
    pub filter_mode: FilterMode,
    pub invert_active: bool,
    pub bits_per_pixel: u8,
    pub capture_state: bool,
    pub portrait_mode: bool,
    pub threshold: u8,
    pub use_high_range: bool,
}

pub struct CaptureResult {
    pub color_image: egui::ColorImage,
    pub packed_data: Vec<u8>,
    pub device_pixels: Vec<u8>,
}

pub struct Capture {
    pub region: Option<CaptureRegion>,
    pub texture: Option<egui::TextureHandle>,
    pub filter_mode: FilterMode,
    pub invert_active: bool,
    pub rx_res: Option<mpsc::Receiver<CaptureResult>>,
    pub tx_stop: Option<mpsc::Sender<()>>,
    pub config: Option<Arc<Mutex<CaptureConfig>>>,
    pub window_pos_initialized: bool,
    pub threshold: u8,
    pub device_pixels: Option<Vec<u8>>,
}

impl Default for Capture {
    fn default() -> Self {
        Self {
            region: None,
            texture: None,
            filter_mode: FilterMode::HighContrast,
            invert_active: false,
            rx_res: None,
            tx_stop: None,
            config: None,
            window_pos_initialized: false,
            threshold: 128,
            device_pixels: None,
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Some(tx) = &self.tx_stop {
            let _ = tx.send(());
        }
    }
}

pub fn process_frame_bg(cfg: &CaptureConfig, monitor: &Monitor) -> Option<CaptureResult> {
    let region = cfg.region.as_ref()?;

    let margin = 2;
    let (cx, cy, cw, ch) = if region.width > margin * 2 && region.height > margin * 2 {
        (
            region.x + margin,
            region.y + margin,
            region.width - margin * 2,
            region.height - margin * 2,
        )
    } else {
        (region.x, region.y, region.width, region.height)
    };

    let image = match monitor.capture_region(cx, cy, cw, ch) {
        Ok(img) => img,
        Err(err) => {
            log::error!("Capture failed: {:?}", err);
            return None;
        }
    };

    let (inter_width, inter_height) = if cfg.portrait_mode {
        (128, 192)
    } else {
        (192, 128)
    };

    let dyn_captured = image::DynamicImage::ImageRgba8(image);
    let resized_rgba = dyn_captured
        .resize(
            inter_width,
            inter_height,
            image::imageops::FilterType::Triangle,
        )
        .to_rgba8();

    let width = resized_rgba.width();
    let height = resized_rgba.height();
    if width == 0 || height == 0 {
        return None;
    }
    let mut current_buffer = resized_rgba.into_raw();
    let mut is_grayscale = false;
    let mut outline_edges: Option<GrayImage> = None;

    if cfg.filter_mode != FilterMode::None {
        let gray_vec: Vec<u8> = current_buffer
            .chunks_exact(4)
            .map(|rgba| {
                ((rgba[0] as u32 * 38 + rgba[1] as u32 * 75 + rgba[2] as u32 * 15) >> 7) as u8
            })
            .collect();
        if let Some(mut gray_image) = GrayImage::from_vec(width, height, gray_vec) {
            is_grayscale = true;
            if cfg.filter_mode == FilterMode::HighContrast {
                gray_image = median_filter(&gray_image, 1, 1);
                threshold_mut(&mut gray_image, cfg.threshold, ThresholdType::Binary);
                dilate_mut(&mut gray_image, Norm::LInf, 1);
                dilate_mut(&mut gray_image, Norm::LInf, 1);
            } else if cfg.filter_mode == FilterMode::Outline {
                let low = cfg.threshold as f32 * 0.4;
                let high = cfg.threshold as f32 * 0.8;
                let mut edge_image = imageproc::edges::canny(&gray_image, low, high);

                // 실제 물리 점자 디스플레이로 보내는 아웃라인은 팽창(dilation)이 배제된 얇은(1px) 원본 윤곽선을 사용합니다.
                outline_edges = Some(edge_image.clone());

                // PC 화면/GUI Preview 상에서는 굵고 선명하게 보이도록 팽창을 처리해 줍니다.
                dilate_mut(&mut edge_image, Norm::LInf, 1);

                // 화면/GUI 상에서는 흰색 배경에 검은색 윤곽선만 보이도록 구성
                let mut output_image = GrayImage::from_pixel(width, height, image::Luma([255]));
                for y in 0..height {
                    for x in 0..width {
                        if *edge_image.get_pixel(x, y) == image::Luma([255]) {
                            output_image.put_pixel(x, y, image::Luma([0]));
                        }
                    }
                }
                gray_image = output_image;
            }
            current_buffer = gray_image.into_raw();
        }
    }

    if cfg.invert_active {
        if is_grayscale {
            current_buffer.iter_mut().for_each(|p| *p = 255 - *p);
        } else {
            current_buffer.chunks_exact_mut(4).for_each(|rgba| {
                rgba[0] = 255 - rgba[0];
                rgba[1] = 255 - rgba[1];
                rgba[2] = 255 - rgba[2];
            });
        }
    }

    let color_image = if cfg.filter_mode != FilterMode::None {
        egui::ColorImage::from_gray([width as usize, height as usize], &current_buffer)
    } else {
        egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &current_buffer)
    };

    let mut dyn_img = if cfg.filter_mode != FilterMode::None {
        image::DynamicImage::ImageLuma8(
            image::GrayImage::from_vec(width, height, current_buffer).unwrap_or_default(),
        )
    } else {
        image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_vec(width, height, current_buffer).unwrap_or_default(),
        )
    };

    if cfg.portrait_mode {
        dyn_img = dyn_img.rotate90();
    }

    let resized = dyn_img
        .resize(48, 32, image::imageops::FilterType::Triangle)
        .to_luma8();
    let mut padded = image::GrayImage::new(48, 32);
    let offset_x = (48 - resized.width()) / 2;
    let offset_y = (32 - resized.height()) / 2;
    if cfg.filter_mode == FilterMode::Outline {
        let bg_val = if cfg.invert_active { 255 } else { 0 };
        padded = image::GrayImage::from_pixel(48, 32, image::Luma([bg_val]));
    } else {
        image::imageops::replace(&mut padded, &resized, offset_x as i64, offset_y as i64);
    }

    if let Some(edges) = outline_edges {
        let mut dyn_edge = image::DynamicImage::ImageLuma8(edges);
        if cfg.portrait_mode {
            dyn_edge = dyn_edge.rotate90();
        }
        let resized_edge = dyn_edge
            .resize(48, 32, image::imageops::FilterType::Triangle)
            .to_luma8();
        let mut padded_edge = image::GrayImage::new(48, 32);
        image::imageops::replace(
            &mut padded_edge,
            &resized_edge,
            offset_x as i64,
            offset_y as i64,
        );

        let edge_val = if cfg.invert_active { 0 } else { 255 };
        for y in 0..32 {
            for x in 0..48 {
                if padded_edge.get_pixel(x, y)[0] > 30 {
                    padded.put_pixel(x, y, image::Luma([edge_val]));
                }
            }
        }
    }

    if cfg.filter_mode == FilterMode::HighContrast {
        let mut thresh = cfg.threshold;
        if cfg.invert_active {
            thresh = 255u8.saturating_sub(thresh);
        }
        threshold_mut(&mut padded, thresh, ThresholdType::Binary);
    }

    let device_pixels = padded.to_vec();

    let bpp = cfg.bits_per_pixel;
    let packed_data = if bpp == 8 {
        padded.into_raw()
    } else {
        let pixels_per_byte = (8 / bpp) as usize;
        let raw = padded.into_raw();
        let mut packed = Vec::with_capacity(raw.len().div_ceil(pixels_per_byte));
        let mut current_byte = 0u8;
        let mut bit_offset = 0;
        for pixel in raw {
            let scaled = if bpp == 4 {
                let level_3bit = ((pixel as u16 * 7 + 127) / 255) as u8; // 0..=7
                if cfg.use_high_range {
                    8 + level_3bit
                } else {
                    level_3bit
                }
            } else {
                let max_val = (1u16 << bpp) - 1;
                ((pixel as u16 * max_val + 127) / 255) as u8
            };
            let max_val = (1u16 << bpp) - 1;
            current_byte |= (scaled & (max_val as u8)) << (8 - bpp - bit_offset);
            bit_offset += bpp;
            if bit_offset >= 8 {
                packed.push(current_byte);
                current_byte = 0;
                bit_offset = 0;
            }
        }
        if bit_offset > 0 {
            packed.push(current_byte);
        }
        packed
    };

    Some(CaptureResult {
        color_image,
        packed_data,
        device_pixels,
    })
}
