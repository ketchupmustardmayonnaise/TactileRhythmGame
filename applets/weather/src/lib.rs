use graphics::{Circle, Draw, Line};
use libm::{cosf, sinf};
use log::info;
use sdk::Applet;
use sdk::Result;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use sdk::applet::SpeechResult;
use sdk::event::UpdateResult;
use std::f32::consts::PI;

/// 기상 코드 매퍼 및 세부 설명
fn get_weather_desc(code: u32) -> &'static str {
    match code {
        0 => "맑음",
        1 => "대체로 맑음",
        2 => "구름 조금",
        3 => "흐림",
        45 | 48 => "안개",
        51 | 53 | 55 => "이슬비",
        61 | 63 | 65 => "비",
        71 | 73 | 75 => "눈",
        80..=82 => "소나기",
        95 => "뇌우",
        _ => "기타 날씨",
    }
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Init,
    LocatingPrimary,
    LocatingBackup,
    FetchingWeather {
        city: String,
        lat: f64,
        lon: f64,
    },
    Ready {
        city: String,
        temp: f64,
        weather_code: u32,
    },
    Error(String),
}

pub struct WeatherApp {
    state: State,
    last_request_id: Option<u32>,
    speech_output: Option<String>,
    monotonic_time_nanos: u64,
}

impl Default for WeatherApp {
    fn default() -> Self {
        Self {
            state: State::Init,
            last_request_id: None,
            speech_output: Some(
                "실시간 날씨 안내 앱을 시작합니다. 현재 위치를 확인하는 중입니다.".to_string(),
            ),
            monotonic_time_nanos: 0,
        }
    }
}

impl WeatherApp {
    fn start_weather_fetch(&mut self, context: &mut Context, city: String, lat: f64, lon: f64) {
        let weather_url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}&current_weather=true",
            lat, lon
        );

        match context.http.fetch_async("GET", &weather_url, &[], &[]) {
            Ok(new_req_id) => {
                self.last_request_id = Some(new_req_id);
                self.state = State::FetchingWeather { city, lat, lon };
                self.speech_output = Some(format!(
                    "위치 확인 완료. {}의 날씨 정보를 조회합니다.",
                    self.state_city().unwrap_or("현재 위치")
                ));
                info!("Weather: Sent fetch request (ID: {})", new_req_id);
            }
            Err(err) => {
                self.state = State::Error(format!("날씨 요청 실패: {}", err));
                self.speech_output = Some("날씨 요청을 전송하지 못했습니다.".to_string());
            }
        }
    }

    fn trigger_backup_locator(&mut self, context: &mut Context) {
        match context
            .http
            .fetch_async("GET", "https://freeipapi.com/api/json", &[], &[])
        {
            Ok(req_id) => {
                self.last_request_id = Some(req_id);
                self.state = State::LocatingBackup;
                info!(
                    "Locating: Sent FreeIPAPI request (ID: {}) as fallback",
                    req_id
                );
            }
            Err(err) => {
                self.state = State::Error(format!("백업 위치 요청 전송 실패: {}", err));
                self.speech_output =
                    Some("위치 확인 요청을 전송하는 중 문제가 발생했습니다.".to_string());
            }
        }
    }

    fn state_city(&self) -> Option<&str> {
        match &self.state {
            State::FetchingWeather { city, .. } => Some(city),
            State::Ready { city, .. } => Some(city),
            _ => None,
        }
    }
}

impl Applet for WeatherApp {
    fn on_start(&mut self, context: &mut Context) -> Result<()> {
        let _ = sdk::api::log::init();
        info!("WeatherApp on_start");

        // Preferences 데이터베이스에서 기존 캐싱된 위치 정보 조회 시도
        if let (Ok(city), Ok(lat), Ok(lon)) = (
            context.preferences.get_string("cached_city"),
            context.preferences.get_float("cached_lat"),
            context.preferences.get_float("cached_lon"),
        ) {
            info!("Cached location found: {} ({}, {})", city, lat, lon);

            // 캐시 데이터가 있다면 즉시 날씨 정보(Open-Meteo)만 비동기로 요청
            self.start_weather_fetch(context, city, lat as f64, lon as f64);
        } else {
            // 캐시가 없을 때만 Geo-IP를 통한 위치 확인을 시작하도록 State::Init 유지
            info!("No cached location. Ready to locate via ipapi.co.");
            self.state = State::Init;
        }

        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> Result<UpdateResult> {
        self.monotonic_time_nanos = context.time.get_monotonic_time().as_nanos() as u64;
        let mut needs_redraw = false;

        // 1. 키패드 조작 처리 (Center를 누르면 위치 획득부터 완전히 새로고침, Up/Down을 누르면 설명 다시 발음)
        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state == KeyState::Pressed {
                match event.code {
                    KeyCode::Center => {
                        self.state = State::Init;
                        self.last_request_id = None;
                        self.speech_output =
                            Some("위치 및 날씨 정보를 다시 새로고침합니다.".to_string());
                        needs_redraw = true;
                    }
                    KeyCode::Up | KeyCode::Down => {
                        match &self.state {
                            State::Ready {
                                city,
                                temp,
                                weather_code,
                            } => {
                                let desc = get_weather_desc(*weather_code);
                                self.speech_output = Some(format!(
                                    "현재 위치는 {}이며, 기온은 {}도, 날씨는 {}입니다.",
                                    city, temp, desc
                                ));
                            }
                            State::LocatingPrimary | State::LocatingBackup => {
                                self.speech_output = Some("현재 위치를 조회 중입니다.".to_string());
                            }
                            State::FetchingWeather { city, .. } => {
                                self.speech_output =
                                    Some(format!("{}의 날씨 정보를 가져오는 중입니다.", city));
                            }
                            State::Error(msg) => {
                                self.speech_output = Some(format!(
                                    "오류가 발생했습니다. {}. 가운데 버튼을 누르면 다시 시도합니다.",
                                    msg
                                ));
                            }
                            _ => {}
                        }
                        needs_redraw = true;
                    }
                    _ => {}
                }
            }
        }

        // 2. 비동기 상태 머신 및 네트워크 응답 처리
        let current_state = self.state.clone();
        match current_state {
            State::Init => {
                // ipapi.co 호출 시작 (우선순위 1)
                match context
                    .http
                    .fetch_async("GET", "https://ipapi.co/json/", &[], &[])
                {
                    Ok(req_id) => {
                        self.last_request_id = Some(req_id);
                        self.state = State::LocatingPrimary;
                        info!("Locating: Sent ipapi.co request (ID: {})", req_id);
                    }
                    Err(err) => {
                        info!(
                            "Locating: ipapi.co fetch_async failed: {}. Falling back to FreeIPAPI.",
                            err
                        );
                        self.trigger_backup_locator(context);
                    }
                }
                needs_redraw = true;
            }
            State::LocatingPrimary => {
                if let Some((status_code, body)) = self
                    .last_request_id
                    .and_then(|id| context.http.take_response(id))
                {
                    info!("LocatingPrimary: Received response status: {}", status_code);
                    if status_code == 200 {
                        let parsed: serde_json::Value =
                            serde_json::from_slice(&body).unwrap_or_default();
                        let city = parsed["city"]
                            .as_str()
                            .unwrap_or("알 수 없는 도시")
                            .to_string();
                        let lat = parsed["latitude"].as_f64().unwrap_or(37.566);
                        let lon = parsed["longitude"].as_f64().unwrap_or(126.978);

                        info!(
                            "LocatingPrimary: Success! City: {}, Lat: {}, Lon: {}",
                            city, lat, lon
                        );

                        // 성공한 위치 정보를 Preferences 보관소에 캐싱 저장
                        let _ = context.preferences.set_string("cached_city", &city);
                        let _ = context.preferences.set_float("cached_lat", lat as f32);
                        let _ = context.preferences.set_float("cached_lon", lon as f32);
                        info!("LocatingPrimary: Location cached in preferences successfully.");

                        self.start_weather_fetch(context, city, lat, lon);
                    } else {
                        info!(
                            "LocatingPrimary: HTTP error {}. Falling back to FreeIPAPI.",
                            status_code
                        );
                        self.trigger_backup_locator(context);
                    }
                    needs_redraw = true;
                }
            }
            State::LocatingBackup => {
                if let Some((status_code, body)) = self
                    .last_request_id
                    .and_then(|id| context.http.take_response(id))
                {
                    info!("LocatingBackup: Received response status: {}", status_code);
                    if status_code == 200 {
                        let parsed: serde_json::Value =
                            serde_json::from_slice(&body).unwrap_or_default();
                        let city = parsed["cityName"]
                            .as_str()
                            .unwrap_or("알 수 없는 도시")
                            .to_string();
                        let lat = parsed["latitude"].as_f64().unwrap_or(37.566);
                        let lon = parsed["longitude"].as_f64().unwrap_or(126.978);

                        info!(
                            "LocatingBackup: Success! City: {}, Lat: {}, Lon: {}",
                            city, lat, lon
                        );

                        // 성공한 위치 정보를 Preferences 보관소에 캐싱 저장
                        let _ = context.preferences.set_string("cached_city", &city);
                        let _ = context.preferences.set_float("cached_lat", lat as f32);
                        let _ = context.preferences.set_float("cached_lon", lon as f32);
                        info!("LocatingBackup: Location cached in preferences successfully.");

                        self.start_weather_fetch(context, city, lat, lon);
                    } else {
                        self.state = State::Error(format!("위치 획득 HTTP 오류: {}", status_code));
                        self.speech_output = Some(
                            "위치 정보를 가져오지 못했습니다. 임시 서버 점검일 수 있습니다."
                                .to_string(),
                        );
                    }
                    needs_redraw = true;
                }
            }
            State::FetchingWeather {
                city,
                lat: _,
                lon: _,
            } => {
                if let Some((status_code, body)) = self
                    .last_request_id
                    .and_then(|id| context.http.take_response(id))
                {
                    info!("Weather: Received response status: {}", status_code);
                    if status_code == 200 {
                        let parsed: serde_json::Value =
                            serde_json::from_slice(&body).unwrap_or_default();
                        let current_weather = &parsed["current_weather"];
                        let temp = current_weather["temperature"].as_f64().unwrap_or(0.0);
                        let weather_code =
                            current_weather["weathercode"].as_i64().unwrap_or(0) as u32;

                        let desc = get_weather_desc(weather_code);
                        self.speech_output = Some(format!(
                            "현재 위치는 {}이며, 기온은 {}도, 날씨는 {}입니다.",
                            city, temp, desc
                        ));
                        self.state = State::Ready {
                            city: city.clone(),
                            temp,
                            weather_code,
                        };
                        self.last_request_id = None;
                    } else {
                        self.state = State::Error(format!("날씨 획득 HTTP 오류: {}", status_code));
                        self.speech_output =
                            Some("날씨 세부 정보를 획득하는 중에 오류가 발생했습니다.".to_string());
                    }
                    needs_redraw = true;
                }
            }
            _ => {}
        }

        let is_animating = match &self.state {
            State::LocatingPrimary | State::LocatingBackup | State::FetchingWeather { .. } => true,
            State::Ready { weather_code, .. } => {
                matches!(*weather_code, 51 | 53 | 55 | 61 | 63 | 65 | 80..=82 | 95)
            }
            _ => false,
        };

        if needs_redraw || is_animating {
            Ok(UpdateResult::NeedsRedraw)
        } else {
            Ok(UpdateResult::Unchanged)
        }
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> Result<()> {
        canvas.clear();
        let size = canvas.get_size();

        // 1. 상태에 따른 텍스트/애니메이션 렌더링
        match &self.state {
            State::LocatingPrimary | State::LocatingBackup => {
                // 위치 확인 중: 구석 점자 로딩 인디케이터 점등 및 좌측에 대기 애니메이션
                let tick = (self.monotonic_time_nanos / 250_000_000) % 4;
                for i in 0..=tick {
                    canvas.set_pin(Point::new(5 + i as i16 * 3, 16), Intensity::MAX);
                }
                // 구석에 로딩 표시
                canvas.set_pin(Point::new(0, 0), Intensity::MAX);
            }
            State::FetchingWeather { .. } => {
                // 날씨 가져오는 중: 하단 로딩 바형태 애니메이션
                let tick = (self.monotonic_time_nanos / 150_000_000) % 8;
                for i in 0..8 {
                    let intensity = if i == tick as i16 {
                        Intensity::MAX
                    } else {
                        Intensity::new(20)
                    };
                    canvas.set_pin(Point::new(20 + i * 2, 28), intensity);
                }
                canvas.set_pin(Point::new(0, 0), Intensity::MAX);
            }
            State::Ready {
                temp, weather_code, ..
            } => {
                // 날씨 정보 그리기
                let weather_code = *weather_code;

                // 2. 우측 온도 인디케이터 바 표시 (범위: -10 ~ 40도)
                let clamp_temp = temp.clamp(-10.0, 40.0);
                let bar_height = (((clamp_temp + 10.0) / 50.0) * (size.height - 4) as f64) as i16;
                let x_bar = size.width - 3;
                for y in 0..bar_height {
                    let y_pos = size.height - 3 - y;
                    canvas.set_pin(Point::new(x_bar, y_pos), Intensity::MAX);
                    canvas.set_pin(Point::new(x_bar + 1, y_pos), Intensity::MAX);
                }

                // 3. 기상 테마 점자 그래픽 아트 렌더링
                let center_x = 22;
                let center_y = 15;
                let center_point = Point::new(center_x, center_y);

                match weather_code {
                    0 | 1 => {
                        // 맑음: 태양 구형 원 + 방사 광선선들
                        Circle::with_center(center_point, 10).draw(canvas);
                        for r in 0..8 {
                            let angle = (r as f32 / 8.0) * 2.0 * PI;
                            let outer = Point::new(
                                center_x + (9.0 * cosf(angle)) as i16,
                                center_y + (9.0 * sinf(angle)) as i16,
                            );
                            let ray = Point::new(
                                center_x + (13.0 * cosf(angle)) as i16,
                                center_y + (13.0 * sinf(angle)) as i16,
                            );
                            Line::new(outer, ray).draw(canvas);
                        }
                    }
                    2 | 3 | 45 | 48 => {
                        // 흐림/안개: 뭉게구름 표현
                        Circle::with_center(Point::new(center_x - 5, center_y + 2), 12)
                            .draw(canvas);
                        Circle::with_center(Point::new(center_x, center_y - 2), 14).draw(canvas);
                        Circle::with_center(Point::new(center_x + 5, center_y + 2), 12)
                            .draw(canvas);
                        // 하단 덮개 평탄선
                        Line::new(
                            Point::new(center_x - 10, center_y + 6),
                            Point::new(center_x + 10, center_y + 6),
                        )
                        .draw(canvas);
                    }
                    51 | 53 | 55 | 61 | 63 | 65 | 80..=82 | 95 => {
                        // 비/소나기/뇌우: 구름 + 낙하하는 사선 빗방울들
                        Circle::with_center(Point::new(center_x - 4, center_y - 3), 10)
                            .draw(canvas);
                        Circle::with_center(Point::new(center_x + 4, center_y - 3), 10)
                            .draw(canvas);
                        Line::new(
                            Point::new(center_x - 8, center_y + 1),
                            Point::new(center_x + 8, center_y + 1),
                        )
                        .draw(canvas);

                        // 아래 빗방울 낙하 핀들
                        let tick = (self.monotonic_time_nanos / 200_000_000) % 3;
                        for &x_drop in &[center_x - 6, center_x - 2, center_x + 2, center_x + 6] {
                            let y_start = center_y + 4 + tick as i16;
                            canvas.set_pin(Point::new(x_drop, y_start), Intensity::MAX);
                            canvas.set_pin(Point::new(x_drop - 1, y_start + 2), Intensity::MAX);
                        }
                    }
                    71 | 73 | 75 => {
                        // 눈: 아름다운 육각형 대칭 결정 구조 아트
                        for r in 0..6 {
                            let angle = (r as f32 / 6.0) * 2.0 * PI;
                            let outer = Point::new(
                                center_x + (11.0 * cosf(angle)) as i16,
                                center_y + (11.0 * sinf(angle)) as i16,
                            );
                            Line::new(center_point, outer).draw(canvas);

                            // 가지치기 핀들
                            let side_angle1 = angle + PI / 3.0;
                            let side_angle2 = angle - PI / 3.0;
                            let branch_point = Point::new(
                                center_x + (7.0 * cosf(angle)) as i16,
                                center_y + (7.0 * sinf(angle)) as i16,
                            );
                            canvas.set_pin(
                                Point::new(
                                    branch_point.x + (3.0 * cosf(side_angle1)) as i16,
                                    branch_point.y + (3.0 * sinf(side_angle1)) as i16,
                                ),
                                Intensity::MAX,
                            );
                            canvas.set_pin(
                                Point::new(
                                    branch_point.x + (3.0 * cosf(side_angle2)) as i16,
                                    branch_point.y + (3.0 * sinf(side_angle2)) as i16,
                                ),
                                Intensity::MAX,
                            );
                        }
                    }
                    _ => {
                        // 기타 날씨: 아름다운 스마일 페이스 아이콘
                        Circle::with_center(center_point, 14).draw(canvas);
                        canvas.set_pin(Point::new(center_x - 3, center_y - 2), Intensity::MAX);
                        canvas.set_pin(Point::new(center_x + 3, center_y - 2), Intensity::MAX);
                        Line::new(
                            Point::new(center_x - 4, center_y + 3),
                            Point::new(center_x + 4, center_y + 3),
                        )
                        .draw(canvas);
                    }
                }
            }
            State::Error(msg) => {
                // 에러 상태: 엑스(X) 마킹 렌더링
                Line::new(Point::new(16, 10), Point::new(32, 22)).draw(canvas);
                Line::new(Point::new(32, 10), Point::new(16, 22)).draw(canvas);
                info!("Drawing error: {}", msg);
            }
            _ => {}
        }

        Ok(())
    }

    fn on_speech(&self, _context: &Context) -> SpeechResult {
        if let Some(ref text) = self.speech_output {
            SpeechResult::text(text.clone())
        } else {
            SpeechResult::None
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::applet::run(Box::new(WeatherApp::default()));
}
