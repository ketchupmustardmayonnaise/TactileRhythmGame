use anyhow::Result;
use crossterm::{
    QueueableCommand, cursor,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use sdk::api::display::{DisplayInterface, Intensity, Point, Size};
use std::io::{Write, stdout};
use std::sync::{Arc, Mutex};

use crate::host::display::HostDisplayInterface;

/// 유니코드 절반 사각형(Half Block)을 사용하여 콘솔에 렌더링하는 디스플레이 구현체입니다.
/// 터미널 조작을 위해 `crossterm` 라이브러리를 사용합니다.
pub struct ConsoleDisplay {
    size: Size,
    /// 각 픽셀(점)의 상태를 저장하는 평면 벡터입니다.
    pins: Arc<Mutex<Vec<u8>>>,
    /// 내부 렌더링 버퍼
    buffer: Vec<u8>,
    /// 버퍼 변경 여부 추적용 플래그
    dirty: bool,
    /// 첫 렌더링 여부를 추적하는 플래그
    first_render: bool,
}

impl ConsoleDisplay {
    /// 지정된 너비와 높이를 가진 새로운 `ConsoleDisplay`를 생성합니다.
    pub fn new(size: Size) -> Self {
        let len = (size.width as usize) * (size.height as usize);
        Self {
            size,
            pins: Arc::new(Mutex::new(vec![0; len])),
            buffer: vec![0; len],
            dirty: false,
            first_render: true,
        }
    }

    /// 외부와 버퍼를 공유하는 형태의 ConsoleDisplay를 생성합니다.
    pub fn new_shared(size: Size, pins: Arc<Mutex<Vec<u8>>>) -> Self {
        let len = (size.width as usize) * (size.height as usize);
        Self {
            size,
            pins,
            buffer: vec![0; len],
            dirty: false,
            first_render: true,
        }
    }
}

impl Drop for ConsoleDisplay {
    fn drop(&mut self) {
        let mut stdout = stdout();
        let _ = stdout.queue(cursor::Show);
        let _ = stdout.queue(ResetColor);
        let _ = stdout.flush();
    }
}

impl DisplayInterface for ConsoleDisplay {
    /// 주어진 (x, y) 좌표에 있는 특정 픽셀의 상태를 설정합니다.
    /// 좌표는 디스플레이 경계 내에 있는지 확인합니다.
    fn set_pin(&mut self, point: Point, intensity: Intensity) {
        let (x, y) = (point.x, point.y);
        let width = self.size.width;

        if x >= 0 && x < width && y >= 0 {
            let idx = (y as usize) * (width as usize) + (x as usize);
            if let Some(pixel) = self.buffer.get_mut(idx) {
                *pixel = intensity.value;
                self.dirty = true;
            }
        }
    }

    /// 디스플레이의 크기를 반환합니다.
    fn get_size(&self) -> Size {
        self.size
    }

    /// 콘솔 렌더링용 픽셀 버퍼를 빠르게 초기화합니다.
    fn clear(&mut self) {
        self.buffer.fill(0);
        self.dirty = true;
    }
}

impl HostDisplayInterface for ConsoleDisplay {
    /// 디스플레이의 현재 상태를 콘솔에 렌더링합니다.
    /// 터미널을 지우고, 테두리를 그리고, 픽셀 상태에 따라 텍스트 그래픽을 그립니다.
    fn show(&mut self) -> Result<()> {
        // 내부 버퍼와 공유 버퍼 동기화 (프레임 당 1회의 락)
        {
            let mut pins = match self.pins.lock() {
                Ok(guard) => guard,
                Err(_) => return Err(anyhow::anyhow!("Display pins Mutex is poisoned")),
            };
            if self.dirty {
                pins.copy_from_slice(&self.buffer);
                self.dirty = false;
            } else {
                self.buffer.copy_from_slice(&pins);
            }
        }

        let mut stdout = stdout();

        // 첫 렌더링 시에만 화면을 지웁니다.
        if self.first_render {
            stdout.queue(Clear(ClearType::All))?;
            self.first_render = false;
        }

        // 커서를 숨기고 왼쪽 상단 모서리로 이동합니다.
        stdout.queue(cursor::Hide)?.queue(cursor::MoveTo(0, 0))?;
        stdout.queue(ResetColor)?;

        let Size { width, height } = self.size;
        let title = format!("Grayscale Display {}x{}", width, height);
        stdout.queue(Print(title))?;

        // 텍스트 블록 문자를 위한 셀 크기를 계산합니다.
        // 각 셀은 가로 1픽셀, 세로 2픽셀(위/아래 절반 사각형)을 나타냅니다.
        let cell_width = width as usize;
        let cell_height = (height as u16).div_ceil(2) as usize;

        // 상단 테두리를 그립니다.
        stdout.queue(cursor::MoveTo(0, 1))?.queue(Print("┌"))?;
        for _ in 0..cell_width {
            stdout.queue(Print("─"))?;
        }
        stdout.queue(Print("┐"))?;

        // 디스플레이의 각 1x2 셀을 순회합니다.
        for y_cell in 0..cell_height {
            stdout
                .queue(cursor::MoveTo(0, y_cell as u16 + 2))?
                .queue(Print("│"))?;

            for x_cell in 0..cell_width {
                let x_base = x_cell as i16;
                let y_base = y_cell as i16 * 2;

                // 상대 좌표에서 핀(픽셀) 상태를 가져오는 헬퍼 함수.
                let get_pin = |x, y| {
                    if x < width && y < height {
                        self.buffer[(y as usize * width as usize) + x as usize]
                    } else {
                        0
                    }
                };

                let top_intensity = get_pin(x_base, y_base);
                let bottom_intensity = get_pin(x_base, y_base + 1);

                // '▀' (U+2580, Upper half block) 문자를 사용합니다.
                // Foreground 색상은 Top 픽셀, Background 색상은 Bottom 픽셀에 매핑합니다.
                stdout.queue(SetForegroundColor(Color::Rgb {
                    r: top_intensity,
                    g: top_intensity,
                    b: top_intensity,
                }))?;

                stdout.queue(SetBackgroundColor(Color::Rgb {
                    r: bottom_intensity,
                    g: bottom_intensity,
                    b: bottom_intensity,
                }))?;

                stdout.queue(Print("▀"))?;
            }

            // 행이 끝날 때 색상을 초기화하고 테두리를 렌더링합니다.
            stdout.queue(ResetColor)?;
            stdout.queue(Print("│"))?;
        }

        // 하단 테두리를 그립니다.
        stdout
            .queue(cursor::MoveTo(0, cell_height as u16 + 2))?
            .queue(Print("└"))?;
        for _ in 0..cell_width {
            stdout.queue(Print("─"))?;
        }
        stdout.queue(Print("┘"))?;
        // 터미널 출력 정리를 위해 디스플레이 아래로 커서를 이동합니다.
        stdout.queue(cursor::MoveTo(0, cell_height as u16 + 3))?;

        // 모든 대기열에 있는 명령을 터미널에 플러시합니다.
        stdout.flush()?;

        Ok(())
    }
}
