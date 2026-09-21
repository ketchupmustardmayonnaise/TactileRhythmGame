use crate::game::Game;
use crate::types::{EditorMode, FunctionKeyFlags, PressedKey};
use physics::InputFlags;
use sdk::api::context::Context;
use sdk::api::keypad::{KeyCode, KeyState, KeypadEvent};

/// 키 이벤트를 처리하여 게임 상태를 업데이트합니다.
pub fn handle_key_event(
    game: &mut Game,
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
pub fn process_input_logic(game: &mut Game, context: &mut Context) {
    let current = game.fn_key_state;
    let prev = game.prev_fn_key_state;
    let just_pressed = current.difference(prev); // (current & !prev)

    // 2. 모드별 단일 키 동작 처리
    // Draw, Erase 모드
    if just_pressed.contains(FunctionKeyFlags::CENTER) {
        // center 키: 픽셀 그리기/지우기 수행 및 현재 상태 백업
        if game.mode == EditorMode::Draw || game.mode == EditorMode::Erase {
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

    if just_pressed.contains(FunctionKeyFlags::FUN) {
        // 모드 변경 (그리기 <-> 지우기)
        let next_tool = if game.mode == EditorMode::Draw {
            EditorMode::Erase
        } else {
            EditorMode::Draw
        };
        game.set_mode(context, next_tool);
    }

    // 3. 현재 상태를 이전 상태로 저장 (다음 프레임 비교용)
    game.prev_fn_key_state = game.fn_key_state;
}

/// 프레임마다 호출되어 키 수명을 관리하고 입력 상태를 갱신합니다.
pub fn update_input_state(game: &mut Game) {
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
