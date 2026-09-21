use crate::game::DrawingGame;
use crate::speak::SpeakEvent;
use crate::types::{EditorMode, FunctionKeyFlags, GameStatus, PressedKey};
use physics::{InertiaMode, InputFlags};
use sdk::api::audio::AudioSegment;
use sdk::api::context::Context;
use sdk::api::display::DisplayInterface;
use sdk::api::keypad::{KeyCode, KeyState, KeypadEvent};
use sdk::tts_segment;

/// 키 이벤트를 처리하여 게임 상태를 업데이트합니다.
pub fn handle_key_event(
    game: &mut DrawingGame,
    context: &mut Context,
    KeypadEvent {
        code: key, state, ..
    }: KeypadEvent,
) {
    // 1. 기능키 입력 버퍼 업데이트
    match key {
        KeyCode::Center | KeyCode::Function | KeyCode::Menu => match state {
            KeyState::Pressed => {
                game.pressed_keys.retain(|k| k.key != Some(key));
                game.pressed_keys.push(PressedKey { key: Some(key) });
            }
            KeyState::Released => {
                game.pressed_keys.retain(|k| k.key != Some(key));
            }
            KeyState::Unknown => {}
        },
        _ => {}
    }

    // 2. 브러시 크기 변경 모드에서의 방향키 처리 (즉시 반응)
    if game.mode == EditorMode::BrushSizeChange {
        if state == KeyState::Pressed {
            match key {
                KeyCode::Up => {
                    game.brush_size = (game.brush_size + 1).min(game.config.on_draw.max_radius);
                    game.update_brush_mask();
                    game.need_redraw = true;

                    // 동일한 크기 변경 안내("크게")가 연속으로 입력되어도 생략되지 않고
                    // 매번 음성 가이드가 출력될 수 있도록 강제 재생(forced) 옵션을 활성화합니다.
                    use sdk::api::audio::Speakable;
                    let text = SpeakEvent::BrushSizeChanged(true).text(game.language);
                    context
                        .audio
                        .speak_text_with_option(&text, sdk::applet::SpeechOption::forced());
                }
                KeyCode::Down => {
                    game.brush_size = (game.brush_size - 1).max(game.config.on_draw.min_radius);
                    game.update_brush_mask();
                    game.need_redraw = true;

                    // 동일한 크기 변경 안내("작게")가 연속으로 입력되어도 생략되지 않고
                    // 매번 음성 가이드가 출력될 수 있도록 강제 재생(forced) 옵션을 활성화합니다.
                    use sdk::api::audio::Speakable;
                    let text = SpeakEvent::BrushSizeChanged(false).text(game.language);
                    context
                        .audio
                        .speak_text_with_option(&text, sdk::applet::SpeechOption::forced());
                }
                _ => {}
            }
        }
        return;
    }

    // 3. 방향키 입력 처리 (스택 관리 - 최근 입력 우선)
    match state {
        KeyState::Pressed => {
            if matches!(
                key,
                KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right
            ) {
                // 1. 이미 눌린 키라면 스택에서 제거 (순서 갱신을 위해 뒤로 보냄)
                game.pressed_keys.retain(|k| k.key != Some(key));

                // 2. 키 스택에 추가
                game.pressed_keys.push(PressedKey { key: Some(key) });

                // 3. 방향키가 3개 이상이면 가장 오래된 방향키 제거 (기능키는 제외하고 카운트)
                let dir_count = game
                    .pressed_keys
                    .iter()
                    .filter(|k| {
                        matches!(
                            k.key,
                            Some(KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right)
                        )
                    })
                    .count();
                if dir_count > game.config.system.max_dir_key_history
                    && let Some(idx) = game.pressed_keys.iter().position(|k| {
                        matches!(
                            k.key,
                            Some(KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right)
                        )
                    })
                {
                    game.pressed_keys.remove(idx);
                }
            }
        }
        KeyState::Released => {
            // 키를 뗐을 때 (버퍼에서 제거)
            if matches!(
                key,
                KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right
            ) {
                // 키 떼면 스택에서 제거
                game.pressed_keys.retain(|k| k.key != Some(key));
            }
        }
        KeyState::Unknown => {}
    }
}

/// 프레임 단위로 호출되어 기능키 조합 및 모드 로직을 처리합니다.
pub fn process_input_logic(game: &mut DrawingGame, context: &mut Context) {
    let current = game.fn_key_state;
    let prev = game.prev_fn_key_state;
    let just_pressed = current.difference(prev); // (current & !prev)
    // Menu 키 : 전체 메뉴 UI 진입
    if just_pressed.contains(FunctionKeyFlags::MENU) {
        if game.mode != EditorMode::BrushSizeChange {
            game.prev_brush_size = game.brush_size;
            game.prev_mode = game.mode;
        }

        game.set_state(context, GameStatus::Menu);
        let size = context.window.get_size();
        // 현재 게임의 에디터 모드(game.mode)를 전달하여 메뉴 시작 도구 위치를 일치시킵니다.
        game.menu.init_canvas(size.width, size.height, game.mode);
        let tool_name = game.menu.items[game.menu.item_bar.selected_index].text(game.language);
        let segments = match game.language {
            sdk::Language::Ko => tts_segment!("도구 선택. ", tool_name),
            sdk::Language::En => tts_segment!("Tool Selection. ", tool_name),
            sdk::Language::Ja => tts_segment!("ツール選択。", tool_name),
        }
        .into_iter()
        .map(AudioSegment::Text)
        .collect::<Vec<_>>();
        // 메뉴 진입 시 현재 선택된 도구의 이름을 한국어 등 해당 언어로 읽어줍니다.
        context
            .audio
            .play_audio_sequence(&segments, sdk::applet::SpeechOption::new());

        game.input_state = InputFlags::empty(); // 이동 멈춤
        game.physics_state.vel = (0.0, 0.0);
        game.pressed_keys.clear();
        game.need_redraw = true;
    }
    // 1. 조합 키 (Modifier) 처리 - 기존 관성 이동 모드 전환 유지
    if current.contains(FunctionKeyFlags::CENTER) && game.input_state.contains(InputFlags::LEFT)
        || game.input_state.contains(InputFlags::RIGHT)
    {
        game.set_movement_mode(context, Box::new(InertiaMode));
    }

    // 2. 모드별 단일 키 동작 처리
    if game.mode == EditorMode::BrushSizeChange {
        if just_pressed.contains(FunctionKeyFlags::CENTER) {
            game.mode = game.prev_mode;
            context
                .audio
                .speak(&SpeakEvent::BrushSizeConfirmed, game.language);

            game.need_redraw = true;
        } else if just_pressed.contains(FunctionKeyFlags::MENU) {
            // Menu 키: 옵션 모드진입
            game.brush_size = game.prev_brush_size;
            game.update_brush_mask();
            let return_mode = game.prev_mode;
            game.set_mode(context, return_mode);
            game.need_redraw = true;
        }
    } else {
        // 그리기 <-> 지우기 모드 변경
        if just_pressed.contains(FunctionKeyFlags::FUN) {
            let next_tool = if game.mode == EditorMode::Draw {
                EditorMode::Erase
            } else {
                EditorMode::Draw
            };
            game.set_mode(context, next_tool);
        }

        // 픽셀 그리기/지우기 수행 및 현재 상태 백업
        if just_pressed.contains(FunctionKeyFlags::CENTER) && game.mode == EditorMode::Draw
            || game.mode == EditorMode::Erase
        {
            // 픽셀을 수정하기 전의 캔버스 상태를 백업
            game.backup_canvas = Some(game.canvas.clone());

            crate::drawing::plot_pixel(
                game,
                game.physics_state.pos.0 as i16,
                game.physics_state.pos.1 as i16,
            );
            game.need_redraw = true;
        }
    }

    // 3. 현재 상태를 이전 상태로 저장 (다음 프레임 비교용)
    game.prev_fn_key_state = game.fn_key_state;
}

/// 프레임마다 호출되어 키 수명을 관리하고 입력 상태를 갱신합니다.
pub fn update_input_state(game: &mut DrawingGame) {
    // 유효한 키들로 입력 상태 재구성
    game.input_state = InputFlags::empty();
    game.fn_key_state = FunctionKeyFlags::empty();

    for k in &game.pressed_keys {
        match k.key {
            Some(KeyCode::Up) => game.input_state.insert(InputFlags::UP),
            Some(KeyCode::Down) => game.input_state.insert(InputFlags::DOWN),
            Some(KeyCode::Left) => game.input_state.insert(InputFlags::LEFT),
            Some(KeyCode::Right) => game.input_state.insert(InputFlags::RIGHT),
            Some(KeyCode::Center) => game.fn_key_state.insert(FunctionKeyFlags::CENTER),
            Some(KeyCode::Function) => game.fn_key_state.insert(FunctionKeyFlags::FUN),
            Some(KeyCode::Menu) => game.fn_key_state.insert(FunctionKeyFlags::MENU),
            _ => {}
        }
    }
}
