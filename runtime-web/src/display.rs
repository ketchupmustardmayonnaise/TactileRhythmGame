use sdk::error::Error;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::CanvasRenderingContext2d;

pub const PIXEL_SCALE: f64 = 10.0;

static HAS_BLINKING_PINS: AtomicBool = AtomicBool::new(false);

thread_local! {
    static PIN_CACHE: RefCell<HashMap<(i32, i32), (u8, bool)>> = RefCell::new(HashMap::new());
    static LAST_BLINK_STATE: RefCell<bool> = const { RefCell::new(false) };
}

pub fn has_blinking_pins() -> bool {
    HAS_BLINKING_PINS.load(Ordering::Relaxed)
}

fn update_blinking_status() {
    let active = PIN_CACHE.with(|cache| cache.borrow().values().any(|&(_, blink)| blink));
    HAS_BLINKING_PINS.store(active, Ordering::Relaxed);
}

pub fn clear_pin_cache() {
    PIN_CACHE.with(|cache| {
        cache.borrow_mut().clear();
    });
    update_blinking_status();
}

fn get_half_period(intensity: u8) -> i64 {
    let level_3bit = ((intensity as u16 * 7 + 127) / 255) as u8; // 0..=7
    match level_3bit {
        1 => 500, // 1Hz -> 500ms ON / 500ms OFF
        2 => 250, // 2Hz -> 250ms ON / 250ms OFF
        3 => 125, // 4Hz -> 125ms ON / 125ms OFF
        4 => 62,  // 8Hz -> 62.5ms ON / 62.5ms OFF
        5 => 31,  // 16Hz -> 31.25ms ON / 31.25ms OFF
        _ => 15,  // 32Hz -> 15.6ms ON / 15.6ms OFF
    }
}

fn draw_single_pin(ctx: &CanvasRenderingContext2d, x: i32, y: i32, intensity: u8, blink: bool) {
    let x_pos = (x as f64) * PIXEL_SCALE;
    let y_pos = (y as f64) * PIXEL_SCALE;

    // Clear previous pin space
    ctx.set_fill_style_str("black");
    ctx.fill_rect(x_pos, y_pos, PIXEL_SCALE, PIXEL_SCALE);

    if blink {
        let level_3bit = ((intensity as u16 * 7 + 127) / 255) as u8; // 0..=7
        if level_3bit == 0 {
            return; // Always down
        }
        if level_3bit < 7 {
            let ms = web_sys::window()
                .and_then(|w| w.performance())
                .map(|p| p.now())
                .unwrap_or(0.0);
            let half_period = get_half_period(intensity);
            let blink_on = (ms as i64 / half_period) % 2 == 0;
            if !blink_on {
                return;
            }
        }
    }

    let c = if blink {
        255
    } else {
        // Quantize to 8 levels (3-bit: 0..=7)
        let level_3bit = ((intensity as u16 * 7 + 127) / 255) as u8;
        ((level_3bit as u16 * 255) / 7) as u8
    };

    if c > 0 {
        let color = format!("rgb({}, {}, {})", c, c, c);
        ctx.set_fill_style_str(&color);
        ctx.begin_path();
        let _ = ctx.arc(
            x_pos + PIXEL_SCALE / 2.0,
            y_pos + PIXEL_SCALE / 2.0,
            PIXEL_SCALE * 0.3,
            0.0,
            std::f64::consts::PI * 2.0,
        );
        ctx.fill();
    }
}

pub fn redraw_pins(ctx: &CanvasRenderingContext2d, force: bool) {
    // If there are blinking pins, we redraw every frame to support multi-frequency animation.
    // Otherwise, we skip redrawing.
    if !force && !has_blinking_pins() {
        return;
    }

    let ms = web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0);

    let canvas = ctx.canvas().unwrap();
    let canvas_width = canvas.width() as f64;
    let canvas_height = canvas.height() as f64;

    ctx.set_fill_style_str("black");
    ctx.fill_rect(0.0, 0.0, canvas_width, canvas_height);

    PIN_CACHE.with(|cache| {
        let cache = cache.borrow();
        for (&(x, y), &(intensity, blink)) in cache.iter() {
            if blink {
                let level_3bit = ((intensity as u16 * 7 + 127) / 255) as u8; // 0..=7
                if level_3bit == 0 {
                    continue; // Always down
                }
                if level_3bit < 7 {
                    let half_period = get_half_period(intensity);
                    let blink_on = (ms as i64 / half_period) % 2 == 0;
                    if !blink_on {
                        continue; // Skip rendering during the 'off' phase of blinking
                    }
                }
            }

            let c = if blink {
                255
            } else {
                // Quantize to 8 levels (3-bit: 0..=7)
                let level_3bit = ((intensity as u16 * 7 + 127) / 255) as u8;
                ((level_3bit as u16 * 255) / 7) as u8
            };

            if c > 0 {
                let x_pos = (x as f64) * PIXEL_SCALE;
                let y_pos = (y as f64) * PIXEL_SCALE;

                let color = format!("rgb({}, {}, {})", c, c, c);
                ctx.set_fill_style_str(&color);
                ctx.begin_path();
                let _ = ctx.arc(
                    x_pos + PIXEL_SCALE / 2.0,
                    y_pos + PIXEL_SCALE / 2.0,
                    PIXEL_SCALE * 0.3,
                    0.0,
                    std::f64::consts::PI * 2.0,
                );
                ctx.fill();
            }
        }
    });
}

pub struct DisplayClosures {
    pub set_pin: Closure<dyn FnMut(i32, i32, i32, bool)>,
}

pub fn setup_imports(
    imports: &js_sys::Object,
    ctx: &CanvasRenderingContext2d,
) -> Result<DisplayClosures, Error> {
    let display = js_sys::Object::new();
    let ctx_clone = ctx.clone();

    let set_pin = Closure::wrap(
        Box::new(move |x: i32, y: i32, intensity: i32, blink: bool| {
            let val = intensity.clamp(0, 255) as u8;

            PIN_CACHE.with(|cache| {
                let mut cache = cache.borrow_mut();
                if val == 0 {
                    cache.remove(&(x, y));
                } else {
                    cache.insert((x, y), (val, blink));
                }
            });

            // Always perform an instant, local drawing update for ultra-high performance
            draw_single_pin(&ctx_clone, x, y, val, blink);

            update_blinking_status();
        }) as Box<dyn FnMut(i32, i32, i32, bool)>,
    );

    js_sys::Reflect::set(
        &display,
        &"set_pin".into(),
        set_pin.as_ref().unchecked_ref(),
    )
    .map_err(|e| Error::ImportError(format!("set_pin: {:?}", e)))?;

    js_sys::Reflect::set(imports, &"display".into(), &display)
        .map_err(|e| Error::ImportError(format!("display: {:?}", e)))?;

    Ok(DisplayClosures { set_pin })
}

/// 패킹된 바이너리 데이터를 받아 지정된 bpp에 따라 압축을 풀고 캔버스에 그립니다.
pub fn draw_packed_data(
    ctx: &CanvasRenderingContext2d,
    width: u32,
    height: u32,
    data: &[u8],
    bpp: u8,
) {
    if bpp == 0 || bpp > 8 {
        return;
    }

    // 전체 배경을 먼저 검은색으로 지워 이전 프레임의 잔상을 제거합니다.
    let canvas_width = (width as f64) * PIXEL_SCALE;
    let canvas_height = (height as f64) * PIXEL_SCALE;
    ctx.set_fill_style_str("black");
    ctx.fill_rect(0.0, 0.0, canvas_width, canvas_height);

    let max_val = (1u32 << bpp) - 1;
    let pixels_per_byte = (8 / bpp) as usize;
    let mut pixel_idx = 0;

    let pi2 = std::f64::consts::PI * 2.0;

    for &byte in data {
        let mut bit_offset = 0;
        for _ in 0..pixels_per_byte {
            if pixel_idx >= (width * height) as usize {
                break; // 남는 패딩 비트는 무시
            }

            // 비트 패킹된 값을 추출하고 0~255 스케일의 밝기(Intensity)로 복원합니다.
            let scaled = (byte >> (8 - bpp - bit_offset)) & (max_val as u8);
            let intensity = ((scaled as u32 * 255) / max_val) as u8;

            // Quantize to 8 levels (3-bit) as implemented on the ESP32
            let level_3bit = ((intensity as u16 * 7 + 127) / 255) as u8;
            let intensity_8level = ((level_3bit as u16 * 255) / 7) as u8;

            if intensity_8level > 0 {
                let x = (pixel_idx % width as usize) as f64;
                let y = (pixel_idx / width as usize) as f64;

                let color =
                    format!("rgb({intensity_8level}, {intensity_8level}, {intensity_8level})");
                ctx.set_fill_style_str(&color);
                ctx.begin_path();
                let _ = ctx.arc(
                    x * PIXEL_SCALE + PIXEL_SCALE / 2.0,
                    y * PIXEL_SCALE + PIXEL_SCALE / 2.0,
                    PIXEL_SCALE * 0.3,
                    0.0,
                    pi2,
                );
                ctx.fill();
            }

            bit_offset += bpp;
            pixel_idx += 1;
        }
    }
}
