use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use embassy_time::{Duration, Timer};

use crate::board_config::{BRAILLE_DISPLAY_HEIGHT, BRAILLE_DISPLAY_WIDTH, FRAME_BUFFER_SIZE};

struct Lcg {
    state: u32,
}

impl Lcg {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        self.state
    }

    fn next_range(&mut self, min: u32, max_inclusive: u32) -> u32 {
        if max_inclusive < min {
            return min;
        }
        min + (self.next() % (max_inclusive - min + 1))
    }
}

// 정수형 제곱근 계산 함수 (원형 도형의 부드러운 외곽선 계산용)
fn isqrt(n: u32) -> u32 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

#[derive(Clone, Copy)]
enum ShapeType {
    Triangle,
    InvertedTriangle,
    Rectangle,
    Star,
    Circle,
    Diamond,
}

#[derive(Clone, Copy, PartialEq)]
enum EffectType {
    Fade,
    Scale,
}

#[derive(Clone, Copy)]
struct Shape {
    active: bool,
    shape_type: ShapeType,
    effect_type: EffectType,
    x: i32,
    y: i32,
    size: i32,
    lifetime: i32,
    max_lifetime: i32,
}

impl Shape {
    fn new_random(rng: &mut Lcg, width: i32, height: i32) -> Self {
        let shape_type = match rng.next_range(0, 5) {
            0 => ShapeType::Triangle,
            1 => ShapeType::InvertedTriangle,
            2 => ShapeType::Rectangle,
            3 => ShapeType::Star,
            4 => ShapeType::Diamond,
            _ => ShapeType::Circle,
        };
        let effect_type = match rng.next_range(0, 1) {
            0 => EffectType::Fade,
            _ => EffectType::Scale,
        };

        let size = rng.next_range(4, 6) as i32;

        Self {
            active: true,
            shape_type,
            effect_type,
            x: rng.next_range(size as u32, (width - 1 - size) as u32) as i32,
            y: rng.next_range(size as u32, (height - 1 - size) as u32) as i32,
            size,
            lifetime: 0,
            max_lifetime: rng.next_range(80, 160) as i32,
        }
    }

    fn intensity(&self) -> u8 {
        if self.effect_type == EffectType::Scale {
            return 255; // 크기가 변하는 도형은 밝기 유지
        }

        let fade_duration = self.max_lifetime / 4;
        if fade_duration == 0 {
            return 255;
        }

        if self.lifetime <= fade_duration {
            ((self.lifetime as u32 * 255) / fade_duration as u32) as u8
        } else if self.lifetime >= self.max_lifetime - fade_duration {
            (((self.max_lifetime - self.lifetime) as u32 * 255) / fade_duration as u32) as u8
        } else {
            255
        }
    }

    // 크기가 부드럽게 변하도록 256을 곱한 고정 소수점(Fixed-point) 형태로 반환합니다.
    fn current_size_fp(&self) -> i32 {
        let fp = 256;
        let size_fp = self.size * fp;

        if self.effect_type == EffectType::Fade {
            return size_fp; // 밝기가 변하는 도형은 최대 크기 유지
        }

        let fade_duration = self.max_lifetime / 4;
        if fade_duration == 0 {
            return size_fp;
        }

        if self.lifetime <= fade_duration {
            (size_fp * self.lifetime) / fade_duration
        } else if self.lifetime >= self.max_lifetime - fade_duration {
            (size_fp * (self.max_lifetime - self.lifetime)) / fade_duration
        } else {
            size_fp
        }
    }

    // 픽셀이 도형에 얼마나 포함되어 있는지를 0~255 강도로 계산 (안티앨리어싱 효과)
    fn coverage(&self, px: i32, py: i32) -> u8 {
        let fp = 256;
        let size_fp = self.current_size_fp();

        let dx = (px - self.x).abs() * fp;
        let dy = (py - self.y) * fp;

        let dist = match self.shape_type {
            ShapeType::Triangle => {
                let d1 = size_fp - dy;
                let d2 = size_fp - (dx * 2 - dy);
                d1.min(d2 / 2) // 기울기에 따른 보정
            }
            ShapeType::InvertedTriangle => {
                let d1 = dy - (-size_fp);
                let d2 = size_fp - (dx * 2 + dy);
                d1.min(d2 / 2)
            }
            ShapeType::Rectangle => size_fp - dx.max(dy.abs()),
            ShapeType::Star => {
                let up_dist = (size_fp / 2 - dy).min((size_fp * 2 - (dx * 3 - dy * 2)) / 3);
                let down_dist = (dy - (-size_fp / 2)).min((size_fp * 2 - (dx * 3 + dy * 2)) / 3);
                up_dist.max(down_dist)
            }
            ShapeType::Circle => {
                let d_sq = (dx as u32 * dx as u32) + (dy.abs() as u32 * dy.abs() as u32);
                size_fp - (isqrt(d_sq) as i32)
            }
            ShapeType::Diamond => (size_fp - (dx + dy.abs())) * 2 / 3,
        };

        if dist <= 0 {
            0
        } else if dist >= fp {
            255
        } else {
            // 도형이 최대 크기(최종 크기)에 도달했을 때는 경계를 부드럽게 하지 않고 뚜렷하게(0 또는 255) 만듭니다.
            if size_fp == self.size * fp {
                if dist >= fp / 2 { 255 } else { 0 }
            } else {
                dist as u8
            }
        }
    }
}

/// 호스트로부터 Stop 요청이 올 때까지 백그라운드에서 물결 모양의 점자 애니메이션을 재생합니다.
pub async fn run(
    buffer_a: &[AtomicU8; FRAME_BUFFER_SIZE],
    buffer_b: &[AtomicU8; FRAME_BUFFER_SIZE],
    active_buffer: &AtomicBool,
    running: &AtomicBool,
) {
    // State for Dynamic animation
    let mut rng = Lcg::new(12345);
    const MAX_SHAPES: usize = 8;
    let mut shapes = [Shape {
        active: false,
        shape_type: ShapeType::Circle,
        effect_type: EffectType::Fade,
        x: 0,
        y: 0,
        size: 0,
        lifetime: 0,
        max_lifetime: 1,
    }; MAX_SHAPES];

    while running.load(Ordering::Relaxed) {
        // 1. 현재 그려지지 않고 있는 백버퍼(Back Buffer) 선택
        let currently_using_b = active_buffer.load(Ordering::Relaxed);
        let back_buffer = if currently_using_b {
            buffer_a
        } else {
            buffer_b
        };

        let w = BRAILLE_DISPLAY_WIDTH as i32;
        let h = BRAILLE_DISPLAY_HEIGHT as i32;

        // Update shapes
        for i in 0..MAX_SHAPES {
            if shapes[i].active {
                shapes[i].lifetime += 1;
                if shapes[i].lifetime >= shapes[i].max_lifetime {
                    shapes[i].active = false;
                }
            } else if rng.next_range(0, 100) < 3 {
                // 매 프레임 빈 슬롯에 3% 확률로 새 도형 생성 시도
                let candidate = Shape::new_random(&mut rng, w, h);
                let mut overlap = false;
                for j in 0..MAX_SHAPES {
                    if shapes[j].active {
                        let dx = (candidate.x - shapes[j].x).abs();
                        let dy = (candidate.y - shapes[j].y).abs();
                        // 각 도형의 크기 합에 약간의 여백(2)을 두어 겹치는지 확인
                        if dx < candidate.size + shapes[j].size + 2
                            && dy < candidate.size + shapes[j].size + 2
                        {
                            overlap = true;
                            break;
                        }
                    }
                }
                // 겹치는 도형이 없을 때만 새 도형을 활성화
                if !overlap {
                    shapes[i] = candidate;
                }
            }
        }

        // Render shapes
        for y in 0..h {
            for x in 0..w {
                let mut max_intensity = 0;
                for shape in shapes.iter() {
                    if shape.active {
                        let coverage = shape.coverage(x, y);
                        if coverage > 0 {
                            let base_intensity = shape.intensity() as u32;
                            // 커버리지(외곽선 투명도)와 도형 고유의 밝기를 곱하여 적용합니다.
                            let intensity = ((coverage as u32 * base_intensity) / 255) as u8;
                            if intensity > max_intensity {
                                max_intensity = intensity;
                            }
                        }
                    }
                }
                let index = (y * w + x) as usize;
                back_buffer[index].store(max_intensity, Ordering::Relaxed);
            }

            // 연산 시간이 길어질 수 있으므로 한 줄(row) 렌더링이 끝날 때마다 다른 태스크에 제어권을 양보합니다.
            embassy_futures::yield_now().await;
        }

        // info!("buffer: {:x}", back_buffer[0].load(Ordering::Relaxed));
        // 2. 백버퍼 렌더링 완료 후 화면 출력 대상 교체 (Swap)
        active_buffer.store(!currently_using_b, Ordering::Release);

        Timer::after(Duration::from_millis(30)).await; // 한 줄당 갱신 속도(애니메이션 속도)
    }

    // 애니메이션 종료 시 모든 픽셀을 0으로 초기화하여 화면(모든 핀)을 내립니다.
    for i in 0..FRAME_BUFFER_SIZE {
        buffer_a[i].store(0, Ordering::Relaxed);
        buffer_b[i].store(0, Ordering::Relaxed);

        // 화면을 지우는 과정에서도 간헐적으로 제어권을 양보합니다.
        if i % 128 == 0 {
            embassy_futures::yield_now().await;
        }
    }
}
