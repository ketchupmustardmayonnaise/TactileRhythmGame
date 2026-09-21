use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use embassy_time::{Duration, Timer};

use crate::board_config::{BRAILLE_DISPLAY_HEIGHT, BRAILLE_DISPLAY_WIDTH, FRAME_BUFFER_SIZE};

/// 호스트로부터 Shutdown 요청이 오면 실행되는 애니메이션입니다.
/// 모든 핀의 강도를 최대(255)로 서서히 올렸다가, 위에서부터 아래로 서서히 지웁니다(0).
pub async fn run(
    buffer_a: &[AtomicU8; FRAME_BUFFER_SIZE],
    buffer_b: &[AtomicU8; FRAME_BUFFER_SIZE],
    active_buffer: &AtomicBool,
    running: &AtomicBool,
) {
    let width = BRAILLE_DISPLAY_WIDTH as usize;
    let height = BRAILLE_DISPLAY_HEIGHT as usize;

    // 1. 모든 핀의 강도를 서서히 올림 (0 -> 255)
    for intensity in (0..=255).step_by(5) {
        if !running.load(Ordering::Relaxed) {
            clear_display(buffer_a, buffer_b);
            return;
        }

        let currently_using_b = active_buffer.load(Ordering::Relaxed);
        let back_buffer = if currently_using_b {
            buffer_a
        } else {
            buffer_b
        };

        for i in 0..FRAME_BUFFER_SIZE {
            back_buffer[i].store(intensity as u8, Ordering::Relaxed);
        }

        active_buffer.store(!currently_using_b, Ordering::Release);
        Timer::after(Duration::from_millis(20)).await;
    }

    // 잠시 최대 강도 유지
    Timer::after(Duration::from_millis(500)).await;

    // 2. 위에서부터 아래로 서서히 강도를 약하게 하여 clear
    let tail_length = 15;
    let total_steps = height + tail_length;

    for step in 0..total_steps {
        if !running.load(Ordering::Relaxed) {
            clear_display(buffer_a, buffer_b);
            return;
        }

        let currently_using_b = active_buffer.load(Ordering::Relaxed);
        let back_buffer = if currently_using_b {
            buffer_a
        } else {
            buffer_b
        };

        for y in 0..height {
            let intensity = if y < step {
                let diff = step - y;
                if diff >= tail_length {
                    0
                } else {
                    ((255 * (tail_length - diff)) / tail_length) as u8
                }
            } else {
                255
            };

            for x in 0..width {
                back_buffer[y * width + x].store(intensity, Ordering::Relaxed);
            }
        }

        active_buffer.store(!currently_using_b, Ordering::Release);
        Timer::after(Duration::from_millis(40)).await;
    }

    // 애니메이션 완료 후 완전히 clear 된 상태 유지
    clear_display(buffer_a, buffer_b);

    // shutdown이 완료될 때까지 대기
    while running.load(Ordering::Relaxed) {
        Timer::after(Duration::from_millis(100)).await;
    }
}

fn clear_display(
    buffer_a: &[AtomicU8; FRAME_BUFFER_SIZE],
    buffer_b: &[AtomicU8; FRAME_BUFFER_SIZE],
) {
    for i in 0..FRAME_BUFFER_SIZE {
        buffer_a[i].store(0, Ordering::Relaxed);
        buffer_b[i].store(0, Ordering::Relaxed);
    }
}
