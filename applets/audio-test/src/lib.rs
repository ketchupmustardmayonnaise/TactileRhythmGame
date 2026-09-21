use sdk::Applet;
use sdk::Result;
use sdk::api::audio::AudioSegment;
use sdk::api::context::Context;
use sdk::api::display::{DisplayInterface, Intensity, Point};
use sdk::api::keypad::{KeyCode, KeyState, KeypadPopResult};
use sdk::applet::SpeechOption;
use sdk::applet::SpeechResult;
use sdk::event::UpdateResult;
use sdk::tts_segment;

// 8000Hz에서 500ms(0.5초) 분량의 샘플 수는 4000개입니다.
const NUM_SAMPLES: usize = 4000;
const WAV_HEADER_SIZE: usize = 44;
const TOTAL_BYTES: usize = WAV_HEADER_SIZE + NUM_SAMPLES;

const BEEP_SOUND_ARRAY: [u8; TOTAL_BYTES] = {
    let mut buf = [0u8; TOTAL_BYTES];

    // RIFF header
    buf[0] = b'R';
    buf[1] = b'I';
    buf[2] = b'F';
    buf[3] = b'F';
    let chunk_size = (36 + NUM_SAMPLES) as u32;
    let cs_bytes = chunk_size.to_le_bytes();
    buf[4] = cs_bytes[0];
    buf[5] = cs_bytes[1];
    buf[6] = cs_bytes[2];
    buf[7] = cs_bytes[3];

    buf[8] = b'W';
    buf[9] = b'A';
    buf[10] = b'V';
    buf[11] = b'E';
    buf[12] = b'f';
    buf[13] = b'm';
    buf[14] = b't';
    buf[15] = b' ';

    // Subchunk1Size (16 for PCM)
    buf[16] = 16;
    buf[17] = 0;
    buf[18] = 0;
    buf[19] = 0;
    // AudioFormat (1 for PCM)
    buf[20] = 1;
    buf[21] = 0;
    // NumChannels (1)
    buf[22] = 1;
    buf[23] = 0;
    // SampleRate (8000)
    buf[24] = 0x40;
    buf[25] = 0x1f;
    buf[26] = 0;
    buf[27] = 0;
    // ByteRate (8000 * 1 * 8 / 8 = 8000)
    buf[28] = 0x40;
    buf[29] = 0x1f;
    buf[30] = 0;
    buf[31] = 0;
    // BlockAlign (1 * 8 / 8 = 1)
    buf[32] = 1;
    buf[33] = 0;
    // BitsPerSample (8)
    buf[34] = 8;
    buf[35] = 0;

    // data chunk
    buf[36] = b'd';
    buf[37] = b'a';
    buf[38] = b't';
    buf[39] = b'a';
    let data_size = NUM_SAMPLES as u32;
    let ds_bytes = data_size.to_le_bytes();
    buf[40] = ds_bytes[0];
    buf[41] = ds_bytes[1];
    buf[42] = ds_bytes[2];
    buf[43] = ds_bytes[3];

    // Generate Sine Wave (10 samples per cycle)
    let pattern = [128, 202, 248, 248, 202, 128, 53, 7, 7, 53];
    let fade_in_samples = 1000; // 처음 1000개 샘플(약 125ms) 동안 페이드 인 적용
    let fade_out_samples = 1000; // 마지막 1000개 샘플(약 125ms) 동안 페이드 아웃 적용
    let mut i = 0;
    while i < NUM_SAMPLES {
        let original_sample = pattern[i % 10];
        let sample = if i < fade_in_samples {
            let centered = original_sample - 128;
            let scaled = (centered * i as i32) / fade_in_samples as i32;
            (128 + scaled) as u8
        } else if i >= NUM_SAMPLES - fade_out_samples {
            let remaining = NUM_SAMPLES - i;
            let centered = original_sample - 128; // 무음(128)을 기준으로 진폭 계산
            let scaled = (centered * remaining as i32) / fade_out_samples as i32;
            (128 + scaled) as u8
        } else {
            original_sample as u8
        };
        buf[44 + i] = sample;
        i += 1;
    }

    buf
};

// 테스트용 WAV 파일 (부드러운 사인파 소리, 8000Hz, 8-bit, 단일 채널, 약 500ms)
const BEEP_SOUND: &[u8] = &BEEP_SOUND_ARRAY;

#[derive(Default)]
pub struct AudioTestApp {
    selected_index: usize,
    trigger_count: u64,
}

impl Applet for AudioTestApp {
    fn on_start(&mut self, _context: &mut Context) -> Result<()> {
        let _ = sdk::api::log::init();
        Ok(())
    }

    fn on_update(&mut self, context: &mut Context) -> sdk::Result<UpdateResult> {
        let mut needs_redraw = false;

        while let KeypadPopResult::Event(event) = context.keypad.pop_event() {
            if event.state == KeyState::Pressed {
                match event.code {
                    KeyCode::Up if self.selected_index > 0 => {
                        self.selected_index -= 1;
                        self.trigger_count = 0; // 메뉴 이동 시 카운트 초기화
                        needs_redraw = true;
                    }
                    KeyCode::Down if self.selected_index < 5 => {
                        self.selected_index += 1;
                        self.trigger_count = 0;
                        needs_redraw = true;
                    }
                    KeyCode::Center => {
                        // 실행 버튼을 누르면 카운트를 올려 상태를 변경합니다.
                        self.trigger_count += 1;

                        // [테스트 4] 단순 효과음은 상태가 아니므로 액션(Action)으로서 직접 play 합니다.
                        if self.selected_index == 4 {
                            context.audio.play(BEEP_SOUND);
                        }
                        // else if self.selected_index == 5 {
                        //     // [테스트 5] 20% 볼륨으로 조절된 효과음 재생
                        //     context.audio.play_with_volume(BEEP_SOUND, 0.2);
                        // }
                        needs_redraw = true;
                    }
                    _ => {}
                }
            }
        }

        if needs_redraw {
            Ok(UpdateResult::NeedsRedraw)
        } else {
            Ok(UpdateResult::Unchanged)
        }
    }

    fn on_draw(&self, canvas: &mut dyn DisplayInterface) -> sdk::Result<()> {
        canvas.clear();

        // 화면에 현재 선택된 테스트 항목을 점자 블록으로 간단히 표시합니다.
        for i in 0..=5 {
            let intensity = if i == self.selected_index {
                Intensity::MAX
            } else {
                Intensity::new(30)
            };
            for x in 2..10 {
                canvas.set_pin(Point::new(x, 2 + i as i16 * 5), intensity);
                canvas.set_pin(Point::new(x, 3 + i as i16 * 5), intensity);
            }
        }
        Ok(())
    }

    fn on_speech(&self, context: &Context) -> SpeechResult {
        // info!("on_speech 호출됨");

        let lang = context.language;

        // 아무런 액션도 취하지 않은 평상시 상태 (메뉴 이동 시 안내 음성)
        if self.trigger_count == 0 {
            let menu_name = match lang {
                sdk::Language::Ko => match self.selected_index {
                    0 => "일반 상태 출력 테스트",
                    1 => "강제 출력 액션 테스트",
                    2 => "중요 메시지 더킹 테스트",
                    3 => "오디오 시퀀스 혼합 테스트",
                    4 => "단순 효과음 중첩 테스트",
                    5 => "단일 효과음 볼륨 조절 테스트",
                    _ => "",
                },
                sdk::Language::En => match self.selected_index {
                    0 => "Normal status output test",
                    1 => "Forced output action test",
                    2 => "Important message ducking test",
                    3 => "Audio sequence mixing test",
                    4 => "Simple sound effect overlapping test",
                    5 => "Single sound effect volume control test",
                    _ => "",
                },
                sdk::Language::Ja => match self.selected_index {
                    0 => "一般状態出力テスト",
                    1 => "強制出力アクションテスト",
                    2 => "重要メッセージダッキングテスト",
                    3 => "オーディオシーケンス混合テスト",
                    4 => "単純効果音重複テスト",
                    5 => "単一効果音音量調節テスト",
                    _ => "",
                },
            };

            let instruction = match lang {
                sdk::Language::Ko => "실행하려면 가운데 버튼을 누르세요",
                sdk::Language::En => "Press the center button to run",
                sdk::Language::Ja => "実行するには中央ボタンを押してください",
            };

            return SpeechResult::segments(
                tts_segment!(menu_name, instruction)
                    .into_iter()
                    .map(AudioSegment::Text)
                    .collect(),
            );
        }

        // 가운데(Center) 버튼을 눌러 상태 변화가 생겼을 때의 선언적 결과 반환
        // i18n::format_num을 사용하여 각 언어에 최적화된 숫자 발음을 가져옵니다.
        let formatted_count = i18n::format_num(lang, self.trigger_count as u32);

        match lang {
            sdk::Language::Ko => match self.selected_index {
                0 => SpeechResult::segments(
                    tts_segment!("일반 메시지입니다. 카운트 ", formatted_count)
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect(),
                ),
                1 => SpeechResult::segments_with_options(
                    tts_segment!("중복을 허용하는 강제 출력입니다. 카운트 ", formatted_count)
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect(),
                    SpeechOption::forced(),
                ),
                2 => SpeechResult::segments_with_options(
                    tts_segment!(
                        "일반 소리를 줄여주는 중요 메시지입니다. 카운트 ",
                        formatted_count
                    )
                    .into_iter()
                    .map(AudioSegment::Text)
                    .collect(),
                    SpeechOption::important(),
                ),
                3 => {
                    // 시퀀스 내에 포함될 시작 메시지를 세그먼트 매크로로 결합합니다.
                    let mut seq = tts_segment!("시퀀스 시작, 카운트 ", formatted_count)
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect::<Vec<_>>();
                    seq.push(AudioSegment::Sound(BEEP_SOUND, 1.0)); // 시퀀스 내장 효과음
                    seq.push(AudioSegment::Silence(500)); // 0.5초 대기
                    seq.push(AudioSegment::Text("테스트 종료.".to_string()));
                    SpeechResult::segments_with_options(seq, SpeechOption::important())
                }
                4 => SpeechResult::segments(
                    tts_segment!("효과음과 함께 출력되는 TTS입니다. 카운트 ", formatted_count)
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect(),
                ),
                5 => SpeechResult::segments(
                    tts_segment!(
                        "20% 볼륨으로 재생되는 효과음입니다. 카운트 ",
                        formatted_count
                    )
                    .into_iter()
                    .map(AudioSegment::Text)
                    .collect(),
                ),
                _ => SpeechResult::None,
            },
            sdk::Language::En => match self.selected_index {
                0 => SpeechResult::segments(
                    tts_segment!("This is a normal message. Count ", formatted_count)
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect(),
                ),
                1 => SpeechResult::segments_with_options(
                    tts_segment!(
                        "This is a forced output allowing overlap. Count ",
                        formatted_count
                    )
                    .into_iter()
                    .map(AudioSegment::Text)
                    .collect(),
                    SpeechOption::forced(),
                ),
                2 => SpeechResult::segments_with_options(
                    tts_segment!(
                        "This is an important message that ducks other sounds. Count ",
                        formatted_count
                    )
                    .into_iter()
                    .map(AudioSegment::Text)
                    .collect(),
                    SpeechOption::important(),
                ),
                3 => {
                    // 시작 메시지를 세그먼트 매크로로 결합합니다.
                    let mut seq = tts_segment!("Sequence start, count ", formatted_count)
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect::<Vec<_>>();
                    seq.push(AudioSegment::Sound(BEEP_SOUND, 1.0));
                    seq.push(AudioSegment::Silence(500));
                    seq.push(AudioSegment::Text("Test ended.".to_string()));
                    SpeechResult::segments_with_options(seq, SpeechOption::important())
                }
                4 => SpeechResult::segments(
                    tts_segment!("TTS outputted with sound effect. Count ", formatted_count)
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect(),
                ),
                5 => SpeechResult::segments(
                    tts_segment!(
                        "Sound effect played at 20 percent volume. Count ",
                        formatted_count
                    )
                    .into_iter()
                    .map(AudioSegment::Text)
                    .collect(),
                ),
                _ => SpeechResult::None,
            },
            sdk::Language::Ja => match self.selected_index {
                0 => SpeechResult::segments(
                    tts_segment!("一般メッセージです。カウント ", formatted_count)
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect(),
                ),
                1 => SpeechResult::segments_with_options(
                    tts_segment!("重複を許容する強制出力です。カウント ", formatted_count)
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect(),
                    SpeechOption::forced(),
                ),
                2 => SpeechResult::segments_with_options(
                    tts_segment!(
                        "一般音量を下げる重要メッセージです。カウント ",
                        formatted_count
                    )
                    .into_iter()
                    .map(AudioSegment::Text)
                    .collect(),
                    SpeechOption::important(),
                ),
                3 => {
                    // 시작 메시지를 세그먼트 매크로로 결합합니다.
                    let mut seq = tts_segment!("シーケンス開始、カウント ", formatted_count)
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect::<Vec<_>>();
                    seq.push(AudioSegment::Sound(BEEP_SOUND, 1.0));
                    seq.push(AudioSegment::Silence(500));
                    seq.push(AudioSegment::Text("テスト終了。".to_string()));
                    SpeechResult::segments_with_options(seq, SpeechOption::important())
                }
                4 => SpeechResult::segments(
                    tts_segment!("効果音と共に出力されるTTSです。カウント ", formatted_count)
                        .into_iter()
                        .map(AudioSegment::Text)
                        .collect(),
                ),
                5 => SpeechResult::segments(
                    tts_segment!(
                        "20パーセントの音量で再生される効果音です。カウント ",
                        formatted_count
                    )
                    .into_iter()
                    .map(AudioSegment::Text)
                    .collect(),
                ),
                _ => SpeechResult::None,
            },
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run() {
    sdk::applet::run(Box::new(AudioTestApp::default()));
}
