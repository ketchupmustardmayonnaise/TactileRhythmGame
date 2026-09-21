use alloc::{format, vec::Vec};
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use embassy_time::Timer;
use esp_hal::{
    Async,
    delay::Delay,
    gpio::{Level, Output, OutputConfig, OutputPin, interconnect::PeripheralOutput},
    spi::master::{Config, Instance, Spi},
};

use crate::board_config::{
    IS_4BIT_MODE, PWM_INTERVAL, PWM_STEP, SPI_FREQUENCY, STROBE_DELAY_MICROS,
};
use crate::{Error, Result};

extern crate alloc;

pub struct BrailleDisplayConfig<'d, SPI, CLOCK, DATA, STROBE> {
    pub spi: SPI,
    pub clock: CLOCK,
    pub data: DATA,
    pub strobe: STROBE,
    pub width: u16,
    pub height: u16,
    pub frame_buffer_a: &'d [AtomicU8],
    pub frame_buffer_b: &'d [AtomicU8],
    pub active_frame_buffer: &'d AtomicBool,
}

pub struct BrailleDisplay<'d> {
    spi_master: Spi<'d, Async>,
    strobe: Output<'d>,
    width: u16,
    height: u16,
    frame_buffer_a: &'d [AtomicU8],
    frame_buffer_b: &'d [AtomicU8],
    active_frame_buffer: &'d AtomicBool,
    spi_payload: Vec<u8>,
    delay: Delay,
    phantom: core::marker::PhantomData<&'d ()>,
}

impl<'d> BrailleDisplay<'d> {
    pub fn new<SPI, CLOCK, DATA, STROBE>(
        config: BrailleDisplayConfig<'d, SPI, CLOCK, DATA, STROBE>,
    ) -> Result<Self>
    where
        SPI: Instance + 'd,
        CLOCK: PeripheralOutput<'d>,
        DATA: PeripheralOutput<'d>,
        STROBE: OutputPin + 'd,
    {
        let spi_master = Self::init_spi_master(config.spi, config.clock, config.data)?;
        let spi_payload = alloc::vec![0; config.width as usize * config.height as usize / 8];
        let strobe = Output::new(config.strobe, Level::Low, OutputConfig::default());
        let delay = Delay::new();

        Ok(Self {
            spi_master,
            strobe,
            width: config.width,
            height: config.height,
            frame_buffer_a: config.frame_buffer_a,
            frame_buffer_b: config.frame_buffer_b,
            active_frame_buffer: config.active_frame_buffer,
            spi_payload,
            delay,
            phantom: core::marker::PhantomData,
        })
    }

    /// 하드웨어 점자 핀을 모두 내리고, 프레임 버퍼를 초기화합니다.
    pub fn clear(&mut self) {
        for i in 0..(self.width as usize * self.height as usize) {
            self.frame_buffer_a[i].store(0, Ordering::Relaxed);
            self.frame_buffer_b[i].store(0, Ordering::Relaxed);
        }
        self.spi_payload.fill(0);
        if let Err(e) = self.update() {
            log::error!("Braille display SPI clear failed: {:?}", e);
        }
    }

    /// 백그라운드에서 무한 루프를 돌며 활성화된 버퍼의 그레이스케일 값을 기반으로
    /// PWM 형태의 점자 밝기(Intensity)를 제어합니다.
    pub async fn draw(&mut self) -> ! {
        let mut counter: u8 = 0;

        loop {
            // 1. 현재 활성화된(화면에 그려야 할) 버퍼 확인 (false = A, true = B)
            let use_b = self.active_frame_buffer.load(Ordering::Acquire);
            let current_buffer = if use_b {
                self.frame_buffer_b
            } else {
                self.frame_buffer_a
            };

            // 2. 픽셀(0~255)과 counter를 비교하여 1-bit spi_payload 생성
            self.spi_payload.fill(0);

            let w = self.width as usize;
            let h = self.height as usize;

            // 루프 내부에서 매번 시간 함수를 호출하지 않도록 프레임 단위로 한 번만 획득합니다.
            let current_us = embassy_time::Instant::now().as_micros();
            let is_4bit = IS_4BIT_MODE.load(Ordering::Relaxed);

            for y in 0..h {
                for x in 0..w {
                    // 원본 프레임 버퍼 인덱스 (왼쪽 위 시작점 기준)
                    let src_idx = y * w + x;
                    let pixel_val = current_buffer[src_idx].load(Ordering::Relaxed);

                    let is_high = if is_4bit {
                        match pixel_val {
                            0 => false,
                            7 => true,
                            1..=6 => {
                                // 1~6 단계 PWM 제어: 0~255 스케일의 counter와 비교
                                // 1 -> 32, 2 -> 64, 3 -> 96, 4 -> 128, 5 -> 160, 6 -> 192의 임계값을 가집니다.
                                let threshold = (pixel_val as u16) * 32;
                                threshold > (counter as u16)
                            }
                            8 => false,
                            15 => true,
                            9..=14 => {
                                // 마이크로초 단위의 타이밍 점멸 제어
                                let interval = match pixel_val {
                                    9 => 500_000,
                                    10 => 250_000,
                                    11 => 125_000,
                                    12 => 62_500,
                                    13 => 31_250,
                                    14 => 15_625,
                                    _ => unreachable!(),
                                };
                                (current_us / interval) % 2 == 0
                            }
                            _ => false,
                        }
                    } else {
                        pixel_val > counter
                    };

                    if is_high {
                        // 하드웨어 점자 핀 인덱스 매핑 (좌우 반전 수정):
                        // 프레임 버퍼의 왼쪽 아래(x = 0, y = h - 1)가 하드웨어의 시작점(index = 0)이 됩니다.
                        let phys_idx = x * h + (h - 1 - y);
                        // 하드웨어 배선에 따라 7 - (phys_idx % 8) 로 변경해야 할 수도 있습니다.
                        self.spi_payload[phys_idx / 8] |= 1 << (7 - (phys_idx % 8));
                    }
                }
            }

            // 3. SPI 전송 및 Strobe 신호 발생
            if let Err(e) = self.update() {
                log::error!("Braille display SPI update failed: {:?}", e);
            }
            counter = counter.wrapping_add(PWM_STEP);

            // 4. 다음 단계로 넘어가기 위한 대기 시간
            Timer::after(PWM_INTERVAL).await;
        }
    }

    pub fn update(&mut self) -> Result<()> {
        self.spi_master
            .write(&self.spi_payload)
            .map_err(|e| Error::SpiError(format!("{:?}", e)))?;
        self.strobe.set_high();
        self.delay.delay_micros(STROBE_DELAY_MICROS);
        self.strobe.set_low();
        Ok(())
    }

    fn init_spi_master<SPI, CLOCK, DATA>(spi: SPI, sclk: CLOCK, sda: DATA) -> Result<Spi<'d, Async>>
    where
        SPI: Instance + 'd,
        CLOCK: PeripheralOutput<'d>,
        DATA: PeripheralOutput<'d>,
    {
        let config: Config = Config::default()
            .with_frequency(SPI_FREQUENCY)
            .with_mode(esp_hal::spi::Mode::_0);
        Ok(Spi::new(spi, config)
            .map_err(|e| Error::SpiError(format!("{:?}", e)))?
            .with_sck(sclk)
            .with_mosi(sda)
            .into_async())
    }
}
