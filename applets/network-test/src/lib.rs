use log::info;
use sdk::Applet;
use sdk::Result;
use sdk::api::audio::AudioSegment;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use sdk::applet::SpeechResult;
use sdk::event::UpdateResult;
use sdk::tts_segment;

const TEST_URLS: &[(&str, &str)] = &[
    ("HTTP GET (httpbin)", "https://httpbin.org/get"),
    ("Delay 2s (httpbin)", "https://httpbin.org/delay/2"),
    ("Teapot 418 (httpbin)", "https://httpbin.org/status/418"),
];

pub struct NetworkTestApp {
    selected_index: usize,
    last_request_id: Option<u32>,
    status_text: String,
    speech_output: Option<Vec<AudioSegment>>,
}

impl Default for NetworkTestApp {
    fn default() -> Self {
        Self {
            selected_index: 0,
            last_request_id: None,
            status_text: "준비 완료".to_string(),
            speech_output: Some(
                tts_segment!(
                    "네트워크 테스트 앱에 진입했습니다. 옵션을 선택하고 가운데 버튼을 누르세요."
                )
                .into_iter()
                .map(AudioSegment::Text)
                .collect(),
            ),
        }
    }
}

impl Applet for NetworkTestApp {
    fn on_start(&mut self, _context: &mut Context) -> Result<()> {
        let _ = sdk::api::log::init();
        info!("NetworkTestApp on_start");
        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        let mut needs_redraw = false;

        // 1. 키패드 입력 처리
        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state == KeyState::Pressed {
                match event.code {
                    KeyCode::Up if self.selected_index > 0 => {
                        self.selected_index -= 1;
                        self.status_text = "준비 완료".to_string();
                        let option_name = TEST_URLS[self.selected_index].0;
                        let segments = match context.language {
                            sdk::types::Language::Ko => tts_segment!("위로 이동. ", option_name),
                            sdk::types::Language::En => tts_segment!("Move up. ", option_name),
                            sdk::types::Language::Ja => tts_segment!("上に移動。 ", option_name),
                        }
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect::<Vec<_>>();
                        self.speech_output = Some(segments);
                        needs_redraw = true;
                    }
                    KeyCode::Down if self.selected_index < TEST_URLS.len() - 1 => {
                        self.selected_index += 1;
                        self.status_text = "준비 완료".to_string();
                        let option_name = TEST_URLS[self.selected_index].0;
                        let segments = match context.language {
                            sdk::types::Language::Ko => tts_segment!("아래로 이동. ", option_name),
                            sdk::types::Language::En => tts_segment!("Move down. ", option_name),
                            sdk::types::Language::Ja => tts_segment!("下に移動。 ", option_name),
                        }
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect::<Vec<_>>();
                        self.speech_output = Some(segments);
                        needs_redraw = true;
                    }
                    KeyCode::Center if self.last_request_id.is_none() => {
                        let url = TEST_URLS[self.selected_index].1;
                        self.status_text = "요청 중...".to_string();
                        let segments = match context.language {
                            sdk::types::Language::Ko => tts_segment!("요청을 시작합니다."),
                            sdk::types::Language::En => tts_segment!("Starting request."),
                            sdk::types::Language::Ja => tts_segment!("リクエストを開始します。"),
                        }
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect::<Vec<_>>();
                        self.speech_output = Some(segments);

                        // 비동기 fetch 호출
                        match context.http.fetch_async(
                            "GET",
                            url,
                            &[("Accept", "application/json")],
                            &[],
                        ) {
                            Ok(req_id) => {
                                self.last_request_id = Some(req_id);
                                info!("Async network request sent. ID: {}", req_id);
                            }
                            Err(err_code) => {
                                self.status_text = format!("오류: {}", err_code);
                                let err_str = err_code.to_string();
                                let segments = match context.language {
                                    sdk::types::Language::Ko => {
                                        tts_segment!("요청 전송 실패. 에러 코드 ", err_str)
                                    }
                                    sdk::types::Language::En => {
                                        tts_segment!("Failed to send request. Error code ", err_str)
                                    }
                                    sdk::types::Language::Ja => {
                                        tts_segment!("リクエスト送信失敗。エラーコード ", err_str)
                                    }
                                }
                                .into_iter()
                                .map(AudioSegment::Text)
                                .collect::<Vec<_>>();
                                self.speech_output = Some(segments);
                            }
                        }
                        needs_redraw = true;
                    }
                    _ => {}
                }
            }
        }

        // 2. 대기 중인 네트워크 비동기 응답 체크 (Polling)
        if let Some(req_id) = self.last_request_id
            && let Some((status_code, body)) = context.http.take_response(req_id)
        {
            self.status_text = format!("HTTP {}", status_code);
            let body_str = String::from_utf8_lossy(&body);
            info!(
                "Async response received. Request ID: {}, Status: {}",
                req_id, status_code
            );
            let preview = body_str
                .char_indices()
                .nth(100)
                .map(|(idx, _)| &body_str[..idx])
                .unwrap_or(&body_str);
            info!("Body preview (100 chars): {}", preview);

            let status_str = status_code.to_string();
            let segments = match context.language {
                sdk::types::Language::Ko => {
                    tts_segment!("HTTP 응답 수신. 상태 코드 ", status_str, "번.")
                }
                sdk::types::Language::En => {
                    tts_segment!("HTTP response received. Status code ", status_str, ".")
                }
                sdk::types::Language::Ja => {
                    tts_segment!("HTTPレスポンス受信。ステータスコード ", status_str, "番。")
                }
            }
            .into_iter()
            .map(AudioSegment::Text)
            .collect::<Vec<_>>();
            self.speech_output = Some(segments);
            self.last_request_id = None;
            needs_redraw = true;
        }

        if needs_redraw {
            Ok(UpdateResult::NeedsRedraw)
        } else {
            Ok(UpdateResult::Unchanged)
        }
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        canvas.clear();

        // 선택된 항목에 따라 특정 핀을 점등하여 렌더링 시각화
        // 간단하게 상하 메뉴 인덱스 표시
        for i in 0..TEST_URLS.len() {
            let intensity = if i == self.selected_index {
                Intensity::MAX
            } else {
                Intensity::new(30)
            };
            for x in 2..15 {
                canvas.set_pin(Point::new(x, 2 + i as i16 * 5), intensity);
                canvas.set_pin(Point::new(x, 3 + i as i16 * 5), intensity);
            }
        }

        // 현재 백그라운드 작업이 돌아가고 있으면 화면 구석에 작은 핀 하나 점등
        if self.last_request_id.is_some() {
            canvas.set_pin(Point::new(0, 0), Intensity::MAX);
        }

        Ok(())
    }

    fn on_speech(&self, _context: &Context) -> SpeechResult {
        if let Some(ref segments) = self.speech_output {
            SpeechResult::segments(segments.clone())
        } else {
            SpeechResult::None
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::applet::run(Box::new(NetworkTestApp::default()));
}
