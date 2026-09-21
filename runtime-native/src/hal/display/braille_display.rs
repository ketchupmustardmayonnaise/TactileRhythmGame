use anyhow::Result;
use protocols::esp32::RequestToEsp32;
use sdk::api::display::{DisplayInterface, Intensity, Point, Size};
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use crate::host::display::HostDisplayInterface;

/// 실제 브라유 디스플레이 하드웨어와의 상호작용을 위한 자리 표시자(placeholder) 구조체입니다.
pub struct BrailleDisplay {
    pub size: Size,
    pub buffer: Vec<Intensity>,
    pub prev_buffer: Vec<Intensity>,
    pub sender: Sender<RequestToEsp32>,
    pub bits_per_pixel: u8,
    pub is_first_frame: bool,
}

impl BrailleDisplay {
    /// 지정된 너비와 높이로 새로운 `BrailleDisplay`를 생성합니다.
    /// 실제 디스플레이 초기화 로직은 여기에 추가될 것입니다.
    pub fn new(size: Size, sender: Sender<RequestToEsp32>, bits_per_pixel: u8) -> Self {
        info!("Initializing Braille Display"); // 초기화 메시지 로깅
        let valid_bpp = if [1, 2, 4, 8].contains(&bits_per_pixel) {
            bits_per_pixel
        } else {
            8
        };
        let len = size.width as usize * size.height as usize;
        Self {
            size,
            buffer: vec![Intensity::default(); len],
            prev_buffer: vec![Intensity::default(); len],
            sender,
            bits_per_pixel: valid_bpp,
            is_first_frame: true,
        }
    }
}

impl DisplayInterface for BrailleDisplay {
    /// 지정된 (x, y) 좌표의 픽셀(핀) 상태를 설정합니다.
    /// 실제 구현에서는 이 메서드가 하드웨어로 명령을 전송하여 핀을 제어합니다.
    fn set_pin(&mut self, point: Point, intensity: Intensity) {
        // 프레임당 수천 번 호출되므로 과도한 로깅을 방지하기 위해 로그를 제거했습니다.
        let (x, y) = (point.x, point.y);
        let width = self.size.width;

        if x >= 0 && x < width && y >= 0 {
            let idx = (y as usize) * (width as usize) + (x as usize);
            if let Some(pixel) = self.buffer.get_mut(idx) {
                *pixel = intensity;
            }
        }
    }

    /// 디스플레이의 크기를 반환합니다.
    fn get_size(&self) -> Size {
        self.size
    }

    /// 디스플레이를 초기화할 때, prev_buffer도 함께 비워 다음 프레임의 불필요한 Diff 전송을 막습니다.
    fn clear(&mut self) {
        self.buffer.fill(Intensity::default());

        if let Err(e) = self.sender.try_send(RequestToEsp32::Clear) {
            tracing::warn!("Failed to send Clear request to ESP32: {}", e);
            // 큐가 가득 차 전송에 실패한 경우 prev_buffer를 비우지 않아, 다음 show() 호출 시 자가 치유되도록 유도합니다.
        } else {
            // 전송 성공 시 prev_buffer도 0으로 비워, 이어지는 show()가 새로 그려진 픽셀들만 효율적으로 전송하게 합니다.
            self.prev_buffer.fill(Intensity::default());
            self.is_first_frame = true;
        }
    }
}

impl HostDisplayInterface for BrailleDisplay {
    /// 디스플레이의 현재 상태를 하드웨어에 반영하여 화면에 표시하도록 트리거합니다.
    /// 실제 구현에서는 이 메서드가 하드웨어 새로고침을 시작합니다.
    fn show(&mut self) -> Result<()> {
        let diff_data = crate::hal::display::calculate_diff(
            &self.prev_buffer,
            &self.buffer,
            self.size.width as u16,
            self.size.height as u16,
            self.bits_per_pixel,
        );

        if diff_data.is_empty() {
            return Ok(());
        }

        let bpp = self.bits_per_pixel;
        let full_frame_size = if bpp == 8 {
            self.buffer.len()
        } else {
            let pixels_per_byte = (8 / bpp) as usize;
            self.buffer.len().div_ceil(pixels_per_byte)
        };

        let is_full_frame = self.is_first_frame || (diff_data.len() * 2 >= full_frame_size);

        let req = if !is_full_frame {
            RequestToEsp32::UpdateDisplayByDiff { data: diff_data }
        } else {
            let packed_data = crate::hal::display::pack_full_frame(&self.buffer, 4);
            RequestToEsp32::UpdateDisplay {
                width: self.size.width as u16,
                height: self.size.height as u16,
                data: packed_data,
            }
        };

        // UART 통신이 막혀있을 때 전체 루프가 블로킹되지 않도록 try_send 사용
        if let Err(e) = self.sender.try_send(req) {
            warn!("Failed to send UpdateDisplay request to ESP32: {}", e);
        } else {
            // 전송에 성공했을 때만 이전 버퍼 상태 업데이트
            self.prev_buffer.copy_from_slice(&self.buffer);
            self.is_first_frame = false;
        }

        Ok(())
    }
}
