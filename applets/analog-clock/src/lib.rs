use core::f32::consts::PI;

use graphics::{Circle, Draw, Line};
use libm::{cosf, sinf};
use sdk::Applet;
use sdk::Language;
use sdk::api::audio::Speakable;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Point};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use i18n::clock; // 공통 다국어 시간 사전 모듈 추가
use sdk::error::Result;
use sdk::event::UpdateResult;

enum Message {
    CurrentTime {
        hours: u64,
        minutes: u64,
        seconds: u64,
    },
}

impl Speakable for Message {
    /// 현재 시간을 다국어 공통 사전 모듈(i18n::clock)을 활용하여 말하기 좋은 발음 가이드 텍스트로 변환합니다.
    /// 한국어의 경우 "한시", "이분" 등과 같이 단위 명사가 자연스럽게 결합된 완성형을 적용하여 음성 간격을 최소화합니다.
    fn text(&self, lang: Language) -> std::borrow::Cow<'_, str> {
        match self {
            Message::CurrentTime {
                hours,
                minutes,
                seconds,
            } => {
                // 공통 사전 모듈을 통해 각 시간 단위를 생성합니다.
                let period_str = clock::period(lang, *hours as u32);
                let hour_str = clock::h(lang, *hours as u32);
                let min_str = clock::m(lang, *minutes as u32);
                let sec_str = clock::s(lang, *seconds as u32);

                match lang {
                    Language::Ko => {
                        // 한국어: "오전 열두시 영분 영초"와 같은 자연스러운 말뭉치 조합을 수행합니다.
                        format!("{} {} {} {}", period_str, hour_str, min_str, sec_str)
                    }
                    _ => {
                        // 영어 및 기타 언어: 시, 분, 초 등을 결합하여 발음 텍스트를 구성합니다.
                        // 예: "It's Two Fifteen PM and thirty seconds"
                        format!(
                            "It's {} {} {} and {}",
                            hour_str, min_str, period_str, sec_str
                        )
                    }
                }
            }
        }
        .into()
    }
}

/// 아날로그 시계 애플릿의 상태를 관리하는 구조체입니다.
#[derive(Default)]
struct AnalogClock {
    last_second: u64, // 마지막으로 그렸을 때의 초 값을 저장하여 불필요한 재그리기를 방지합니다.
    time_seconds: u64, // 렌더링에 사용할 현지 시간(초)을 저장합니다.
    current_time: Option<Message>,
}

impl AnalogClock {
    /// 새로운 `AnalogClock` 인스턴스를 생성합니다.
    fn new() -> Self {
        Self {
            last_second: 60, // 초기값을 60으로 설정하여 처음에 무조건 그려지도록 합니다.
            time_seconds: 0,
            current_time: None,
        }
    }
}

/// `sdk::Applet` 트레이트를 `AnalogClock` 구조체에 구현합니다.
/// 이는 `runtime` 크레이트에서 `AnalogClock` 인스턴스를 WASM 애플릿으로 실행할 수 있도록 합니다.
impl Applet for AnalogClock {
    /// 애플릿의 업데이트 로직을 실행합니다. 내부 `on_update` 메서드를 호출합니다.
    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        // 1. 키패드 이벤트 처리
        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state == KeyState::Pressed && event.code == KeyCode::Function {
                // 현재 시간을 음성으로 안내
                let utc_seconds = context.time.get_time_seconds();
                let offset_seconds = context.time.get_timezone_offset_seconds();
                let local_seconds = (utc_seconds as i64 + offset_seconds as i64) as u64;

                let hours = (local_seconds / 3600) % 24;
                let minutes = (local_seconds / 60) % 60;
                let seconds = local_seconds % 60;

                // Message::CurrentTime의 Speakable 구현을 직접 사용하여 텍스트 가공 중복을 원천 제거합니다.
                let msg = Message::CurrentTime {
                    hours,
                    minutes,
                    seconds,
                };
                // 소유권 문제를 예방하기 위해 Cow 반환값을 소유한 String(.into_owned())으로 복사하여
                // msg에 대한 대여(Borrow) 상태를 해제한 뒤 msg의 소유권을 이동시킵니다.
                let time_string = msg.text(context.language).into_owned();
                self.current_time = Some(msg);

                context.audio.speak_text(&time_string);
            }
        }

        // 2. 시계 업데이트 로직 (기존 로직 유지)
        let current_second = context.time.get_time_seconds() % 60;
        let utc_seconds = context.time.get_time_seconds();
        let offset_seconds = context.time.get_timezone_offset_seconds();
        self.time_seconds = (utc_seconds as i64 + offset_seconds as i64) as u64;

        if current_second != self.last_second {
            self.last_second = current_second;
            Ok(UpdateResult::NeedsRedraw) // 초가 변경되었으므로 다시 그립니다.
        } else {
            Ok(UpdateResult::Unchanged) // 초가 변경되지 않았으므로 다시 그릴 필요가 없습니다.
        }
    }

    /// 애플릿의 그리기 로직을 실행합니다. 내부 `on_draw` 메서드를 호출합니다.
    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        // 디스플레이를 검정색으로 지웁니다.
        canvas.clear();

        // 디스플레이의 크기와 중심, 반지름을 계산합니다.
        let size = canvas.get_size();
        let center = Point::new(size.width / 2, size.height / 2);
        let diameter = size.width.min(size.height);
        let radius = diameter / 2;

        // 시계 얼굴(원)을 그립니다.
        Circle::with_center(center, diameter).draw(canvas);

        // 시간 마커(숫자 대신 짧은 선)를 그립니다.
        for hour in 1..=12 {
            // 각 시간 마커의 각도를 계산합니다. (PI/2.0는 12시 방향을 위로 정렬하기 위함)
            let angle = (hour as f32 / 12.0) * 2.0 * PI - PI / 2.0;
            // 바깥쪽 끝점과 안쪽 끝점을 계산합니다.
            let outer_point = Point::new(
                center.x + (radius as f32 * cosf(angle)) as i16,
                center.y + (radius as f32 * sinf(angle)) as i16,
            );
            let inner_point = Point::new(
                center.x + ((radius - 2) as f32 * cosf(angle)) as i16,
                center.y + ((radius - 2) as f32 * sinf(angle)) as i16,
            );
            // 선을 그리고 디스플레이에 그립니다.
            Line::new(inner_point, outer_point).draw(canvas);
        }

        let time = self.time_seconds;
        // 현재 시간(초)에서 초, 분, 시를 계산합니다.
        let seconds = time % 60;
        let minutes = (time / 60) % 60;
        let hours = (time / 3600) % 12; // 12시간 형식으로 변환합니다.

        // 시침을 그립니다.
        // 시침의 각도를 계산합니다. 분에 따라 시침도 움직여야 합니다.
        let hour_angle = (hours as f32 + minutes as f32 / 60.0) / 12.0 * 2.0 * PI - PI / 2.0;
        let hour_hand_end = Point::new(
            center.x + (radius as f32 * 0.5 * cosf(hour_angle)) as i16, // 반지름의 50% 길이
            center.y + (radius as f32 * 0.5 * sinf(hour_angle)) as i16,
        );
        Line::new(center, hour_hand_end).draw(canvas);

        // 분침을 그립니다.
        // 분침의 각도를 계산합니다. 초에 따라 분침도 움직여야 합니다.
        let minute_angle = (minutes as f32 + seconds as f32 / 60.0) / 60.0 * 2.0 * PI - PI / 2.0;
        let minute_hand_end = Point::new(
            center.x + (radius as f32 * 0.8 * cosf(minute_angle)) as i16, // 반지름의 80% 길이
            center.y + (radius as f32 * 0.8 * sinf(minute_angle)) as i16,
        );
        Line::new(center, minute_hand_end).draw(canvas);

        // 초침을 그립니다.
        // 초침의 각도를 계산합니다.
        let second_angle = seconds as f32 / 60.0 * 2.0 * PI - PI / 2.0;
        let second_hand_end = Point::new(
            center.x + (radius as f32 * 0.9 * cosf(second_angle)) as i16, // 반지름의 90% 길이
            center.y + (radius as f32 * 0.9 * sinf(second_angle)) as i16,
        );
        Line::new(center, second_hand_end).draw(canvas);

        Ok(())
    }

    fn on_speech(&self, context: &Context) -> sdk::applet::SpeechResult {
        if let Some(current_time) = &self.current_time {
            sdk::applet::SpeechResult::text(current_time.text(context.language))
        } else {
            sdk::applet::SpeechResult::None
        }
    }
}

/// WASM 런타임에서 호출되는 애플릿의 메인 진입점 `run` 함수입니다.
/// 이 함수는 애플릿을 초기화하고 SDK에 등록합니다.
#[unsafe(no_mangle)] // 이름 맹글링(name mangling)을 방지하여 C 호환 링크를 가능하게 합니다.
pub extern "C" fn run() {
    sdk::run(Box::new(AnalogClock::new()));
}
