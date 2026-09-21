/// 디스플레이 관련 로직을 그룹화하는 모듈입니다.
pub mod braille_display; // 실제 브라유 디스플레이 하드웨어와 상호작용하는 모듈
pub mod console_display; // 콘솔에 브라유 디스플레이를 에뮬레이션하는 모듈

/// 패킹된 데이터를 8비트 버퍼로 언패킹합니다.
pub fn unpack_frame(data: &[u8], width: u16, height: u16, bits_per_pixel: u8) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let bpp = bits_per_pixel;
    let mut current_buffer = vec![0u8; w * h];

    if bpp == 8 {
        let len = std::cmp::min(current_buffer.len(), data.len());
        current_buffer[..len].copy_from_slice(&data[..len]);
    } else {
        let max_val = (1u16 << bpp) - 1;
        let scale_factor = (255 / max_val) as u8;
        let pixels_per_byte = (8 / bpp) as usize;
        let mut pixel_idx = 0;
        for &byte in data {
            for p in 0..pixels_per_byte {
                if pixel_idx >= current_buffer.len() {
                    break;
                }
                let shift = 8 - bpp - (p as u8 * bpp);
                let val = (byte >> shift) & (max_val as u8);
                current_buffer[pixel_idx] = val * scale_factor;
                pixel_idx += 1;
            }
        }
    }
    current_buffer
}

use sdk::api::display::Intensity;

fn map_intensity_to_4bit(intensity: &Intensity) -> u8 {
    let level_3bit = ((intensity.value as u16 * 7 + 127) / 255) as u8; // 0..=7
    if intensity.blink {
        8 + level_3bit // 8..=15
    } else {
        level_3bit // 0..=7
    }
}

/// 이전 프레임과 현재 프레임을 비교하여 변경된 픽셀의 패킹된 Diff 배열을 생성합니다.
pub fn calculate_diff(
    prev_buffer: &[Intensity],
    current_buffer: &[Intensity],
    width: u16,
    height: u16,
    bits_per_pixel: u8,
) -> Vec<u16> {
    let w = width as usize;
    let h = height as usize;
    let bpp = bits_per_pixel;

    let mut diff_data = Vec::new();

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;

            let cur_val = if bpp == 4 {
                map_intensity_to_4bit(&current_buffer[idx])
            } else {
                let max_val = (1u16 << bpp) - 1;
                ((current_buffer[idx].value as u16 * max_val + 127) / 255) as u8
            };

            let prev_val = if bpp == 4 {
                map_intensity_to_4bit(&prev_buffer[idx])
            } else {
                let max_val = (1u16 << bpp) - 1;
                ((prev_buffer[idx].value as u16 * max_val + 127) / 255) as u8
            };

            if cur_val != prev_val {
                let final_intensity = if bpp == 4 {
                    cur_val
                } else {
                    let max_val = (1u16 << bpp) - 1;
                    let scale_factor = (255 / max_val) as u8;
                    cur_val * scale_factor
                };
                // Bit-packing: x (6 bit, 0~63) | y (6 bit, 0~63) | intensity (4 bit, 0~15)
                let x_pack = (x as u16) & 0x3F;
                let y_pack = (y as u16) & 0x3F;
                let i_pack = if bpp == 4 {
                    cur_val as u16
                } else {
                    (final_intensity as u16) >> 4
                };
                diff_data.push((x_pack << 10) | (y_pack << 4) | i_pack);
            }
        }
    }
    diff_data
}

/// 8비트 버퍼를 주어진 bits_per_pixel에 맞게 패킹합니다.
pub fn pack_full_frame(buffer: &[Intensity], bits_per_pixel: u8) -> Vec<u8> {
    let bpp = bits_per_pixel;
    if bpp == 8 {
        return buffer.iter().map(|i| i.value).collect();
    }
    let max_val = (1u16 << bpp) - 1;
    let pixels_per_byte = (8 / bpp) as usize;
    let full_frame_size = buffer.len().div_ceil(pixels_per_byte);

    let mut packed = Vec::with_capacity(full_frame_size);
    let mut current_byte = 0u8;
    let mut bit_offset = 0;
    for &intensity in buffer {
        let scaled = if bpp == 4 {
            map_intensity_to_4bit(&intensity)
        } else {
            ((intensity.value as u16 * max_val + 127) / 255) as u8
        };
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
}
