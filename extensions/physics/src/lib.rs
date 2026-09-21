use bitflags::bitflags;

bitflags! {
    /// 방향키 입력 상태를 나타내는 비트 플래그입니다.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct InputFlags: u8 {
        const UP = 1 << 0;
        const DOWN = 1 << 1;
        const LEFT = 1 << 2;
        const RIGHT = 1 << 3;
    }
}

impl InputFlags {
    /// 중심 좌표 (0, 0)을 기준으로 입력되는 키 조합에 따른 논리적 이동 단위 벡터 (dx, dy)를 반환합니다.
    pub fn as_unit_vector(&self) -> (f32, f32) {
        let mut dx = 0.0;
        let mut dy = 0.0;

        if self.contains(Self::UP) {
            dy -= 1.0;
        }
        if self.contains(Self::DOWN) {
            dy += 1.0;
        }
        if self.contains(Self::LEFT) {
            dx -= 1.0;
        }
        if self.contains(Self::RIGHT) {
            dx += 1.0;
        }

        (dx, dy)
    }
}

/// 물리 연산 관련 설정
#[derive(Debug, Clone, Copy)]
pub struct PhysicsConfig {
    pub accel_force: i32,      // 가속도 힘
    pub diagonal_accel: i32,   // 대각선 가속도
    pub accel_scale: f32,      // 가속도 스케일
    pub friction_coeff: f32,   // 마찰 계수
    pub velocity_epsilon: f32, // 속도 엡실론 (이 값 이하의 속도는 0으로 처리)
    pub standard_speed: f32,   // 표준 이동 모드 속도
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            accel_force: 1,
            diagonal_accel: 1,
            accel_scale: 0.5,
            friction_coeff: 0.9,
            velocity_epsilon: 0.01,
            standard_speed: 4.0,
        }
    }
}

// 물리 상태 구조체
#[derive(Clone, Copy, Debug)]
pub struct PhysicsState {
    pub pos: (i32, i32),    // 화면에 그려질 정확한 정수 픽셀 위치
    pub rem: (f32, f32),    // 1픽셀 미만의 이동 잔차 (-1.0 ~ 1.0)
    pub vel: (f32, f32),    // 속도
    pub bounds: (i32, i32), // 캔버스 크기 - 화면이탈방지
}

impl PhysicsState {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            pos: (0, 0),
            rem: (0.0, 0.0),
            vel: (0.0, 0.0),
            bounds: (width, height),
        }
    }
}

/// 물리 이동 업데이트 트레이트 - 이동 모드별로 구현
pub trait MovementMode: Send {
    fn on_update(
        &mut self,
        state: &mut PhysicsState,
        input: InputFlags,
        config: &PhysicsConfig,
        dt_ms: f32,
    );
}

/// 이동 모드를 관리하고 실행하는 컨트롤러
pub struct MovementController {
    current_mode: Box<dyn MovementMode>,
}

impl MovementController {
    pub fn new(mode: Box<dyn MovementMode>) -> Self {
        Self { current_mode: mode }
    }

    pub fn on_update(
        &mut self,
        state: &mut PhysicsState,
        input: InputFlags,
        config: &PhysicsConfig,
        dt_ms: f32,
    ) {
        self.current_mode.on_update(state, input, config, dt_ms);
    }

    pub fn switch_mode(&mut self, mode: Box<dyn MovementMode>) {
        self.current_mode = mode;
    }
}

/// 입력 상태 플래그를 기반으로 가속도 벡터(f32)를 반환합니다.
fn get_input_vector(input_state: InputFlags, config: &PhysicsConfig) -> (f32, f32) {
    let (dx, dy) = input_state.as_unit_vector();

    // 대각선 이동 시 보정된 가속도 적용, 직선 이동 시 일반 가속도 적용
    let (ax, ay) = if dx != 0.0 && dy != 0.0 {
        (
            dx * config.diagonal_accel as f32,
            dy * config.diagonal_accel as f32,
        )
    } else {
        (
            dx * config.accel_force as f32,
            dy * config.accel_force as f32,
        )
    };

    // 정수 가속도를 f32 픽셀 단위로 변환 (설정된 스케일 사용)
    (ax * config.accel_scale, ay * config.accel_scale)
}

/// 관성 이동 모드: 가속도와 마찰력을 적용하여 얼음 위처럼 미끄러지는 이동
pub struct InertiaMode;

impl MovementMode for InertiaMode {
    fn on_update(
        &mut self,
        state: &mut PhysicsState,
        input: InputFlags,
        config: &PhysicsConfig,
        _dt_ms: f32,
    ) {
        let (ax, ay) = get_input_vector(input, config);
        let (mut vx, mut vy) = state.vel;
        let (mut px, mut py) = state.pos;
        let (mut rx, mut ry) = state.rem;

        // 1단계: 물리 탱크 (Continuous Float)
        // 가속도 적용
        vx += ax;
        vy += ay;

        // 마찰 적용 (항상 적용)
        vx *= config.friction_coeff;
        vy *= config.friction_coeff;

        // 미세 속도 제거 및 정지 시 누적기 초기화 (깔끔한 출발을 위해)
        if vx.abs() <= config.velocity_epsilon {
            vx = 0.0;
            if ax == 0.0 {
                rx = 0.0;
            }
        }
        if vy.abs() <= config.velocity_epsilon {
            vy = 0.0;
            if ay == 0.0 {
                ry = 0.0;
            }
        }

        // 2단계: 에너지 누적기 (Accumulator)
        // 소수점 잔차에 속도 누적
        rx += vx;
        ry += vy;

        // 3단계: 원자적 격자 이동 (Atomic Grid Step)
        let mut move_x = 0;
        let mut move_y = 0;

        // 대각선 이동 여부 판단
        let is_diagonal = vx.abs() > 0.0 && vy.abs() > 0.0;

        if is_diagonal {
            // 대각선 이동 중일 때는 둘 다 1.0 이상일 때만 동시에 갱신하여 지그재그 방지
            if rx.abs() >= 1.0 && ry.abs() >= 1.0 {
                move_x = rx.trunc() as i32;
                move_y = ry.trunc() as i32;
            } else if rx.abs() >= 2.0 || ry.abs() >= 2.0 {
                // 한쪽 축 속도가 압도적으로 빠를 때 무한 대기를 방지하기 위한 예외 처리
                if rx.abs() >= 1.0 {
                    move_x = rx.trunc() as i32;
                }
                if ry.abs() >= 1.0 {
                    move_y = ry.trunc() as i32;
                }
            }
        } else {
            // 직선 이동 중일 때는 각각 독립적으로 1.0 이상일 때 갱신
            if rx.abs() >= 1.0 {
                move_x = rx.trunc() as i32;
            }
            if ry.abs() >= 1.0 {
                move_y = ry.trunc() as i32;
            }
        }

        // 추출한 정수만큼 잔차 차감 및 실제 위치 업데이트
        rx -= move_x as f32;
        ry -= move_y as f32;
        px += move_x;
        py += move_y;

        // 경계 체크
        let (max_x, max_y) = state.bounds;
        let clamped_x = px.clamp(0, max_x);
        if px != clamped_x {
            px = clamped_x;
            rx = 0.0;
            vx = 0.0;
        }
        let clamped_y = py.clamp(0, max_y);
        if py != clamped_y {
            py = clamped_y;
            ry = 0.0;
            vy = 0.0;
        }

        state.pos = (px, py);
        state.rem = (rx, ry);
        state.vel = (vx, vy);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridMoveState {
    #[default]
    Idle,
    Tolerance, // 대각선 등 다중 키 조합을 식별하기 위한 입력 유예 단계
    Delay,     // 최초 1칸 이동 후, 연속 이동이 시작되기 전 홀드 대기 단계
    Repeating, // 누적 시간에 비례하여 연속으로 픽셀(그리드)을 이동하는 단계
}

/// 단위 벡터를 기반으로 한 칸씩 이동하며 계속 누르고 있으면 시간 비례 가속하는 그리드 이동 모드
#[derive(Default)]
pub struct StepAndAccelerateMode {
    state: GridMoveState,
    ticks: u32,
    locked_input: InputFlags,
    last_raw_input: InputFlags,
    accumulated_steps: f32, // 소수점 단위 누적 이동량 (시간 비례 스텝 추출용)
}

impl StepAndAccelerateMode {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MovementMode for StepAndAccelerateMode {
    fn on_update(
        &mut self,
        state: &mut PhysicsState,
        input: InputFlags,
        _config: &PhysicsConfig,
        _dt_ms: f32,
    ) {
        // 입력이 없으면 상태 초기화
        if input.is_empty() {
            self.state = GridMoveState::Idle;
            self.ticks = 0;
            self.locked_input = InputFlags::empty();
            self.last_raw_input = InputFlags::empty();
            self.accumulated_steps = 0.0;
            state.vel = (0.0, 0.0);
            return;
        }

        let tolerance_ticks = 3; // 방향 확정 유예 프레임
        let delay_ticks = 15; // 최초 한 칸 이동 후 딜레이 프레임

        // 입력 조합이 변경되면 유예 단계부터 다시 시작
        if input != self.last_raw_input {
            self.state = GridMoveState::Tolerance;
            self.ticks = 0;
            self.locked_input = input;
        } else if self.state == GridMoveState::Tolerance {
            self.locked_input |= input;
        }

        self.last_raw_input = input;

        // (0, 0) 중심 기준의 단위 벡터 획득
        let (dx, dy) = self.locked_input.as_unit_vector();
        if dx == 0.0 && dy == 0.0 {
            return;
        }

        let mut target_px = state.pos.0;
        let mut target_py = state.pos.1;

        match self.state {
            GridMoveState::Idle => {
                self.state = GridMoveState::Tolerance;
                self.ticks = 0;
            }
            GridMoveState::Tolerance => {
                if self.ticks >= tolerance_ticks {
                    // 1단계: 유예 시간이 지나면 단위 벡터 방향으로 정확히 1칸 즉시 이동
                    target_px += dx as i32;
                    target_py += dy as i32;

                    self.state = GridMoveState::Delay;
                    self.ticks = 0;
                }
            }
            GridMoveState::Delay => {
                if self.ticks >= delay_ticks {
                    // 2단계: 대기 시간이 지나면 연속 가속 이동 시작
                    self.state = GridMoveState::Repeating;
                    self.ticks = 0;
                    self.accumulated_steps = 0.0;
                }
            }
            GridMoveState::Repeating => {
                // 3단계: 누적 시간에 비례한 점진적 이동량 증가 (시간 비례 가속)
                let base_speed = 0.1; // 프레임당 기본 이동량
                let acceleration = self.ticks as f32 * 0.005; // 유지 시간에 비례해 가속
                let max_speed = 0.5; // 프레임당 최대 이동 칸 제한

                let current_speed = (base_speed + acceleration).min(max_speed);

                // 대각선 이동 시 단위 벡터 크기 보정
                let step_amount = if dx != 0.0 && dy != 0.0 {
                    current_speed * std::f32::consts::FRAC_1_SQRT_2
                } else {
                    current_speed
                };

                self.accumulated_steps += step_amount;

                // 이동량이 1칸(그리드) 이상 모이면 정수 칸수만큼 추출하여 이동
                if self.accumulated_steps >= 1.0 {
                    let step_count = self.accumulated_steps.floor() as i32;
                    self.accumulated_steps -= step_count as f32;

                    target_px += (dx as i32) * step_count;
                    target_py += (dy as i32) * step_count;
                }
            }
        }

        // 화면 이탈 방지 및 최종 위치 갱신
        let (max_x, max_y) = state.bounds;
        state.pos = (target_px.clamp(0, max_x), target_py.clamp(0, max_y));
        state.rem = (0.0, 0.0); // 좌표 기반 그리드 이동이므로 소수점 잔차는 항상 0으로 유지

        // 프레임 카운터 증가
        self.ticks += 1;
    }
}

/// 표준 이동 모드: 등속도 이동 (입력 시 즉시 이동, 해제 시 즉시 정지)
pub struct StandardMode;

impl MovementMode for StandardMode {
    fn on_update(
        &mut self,
        state: &mut PhysicsState,
        input: InputFlags,
        config: &PhysicsConfig,
        _dt_ms: f32,
    ) {
        let (dx, dy) = input.as_unit_vector();

        // 대각선 이동 시 속도 보정 (1 / sqrt(2) ≈ 0.7071)
        let speed = if dx != 0.0 && dy != 0.0 {
            config.standard_speed * std::f32::consts::FRAC_1_SQRT_2
        } else {
            config.standard_speed
        };
        let (vx, vy) = (dx * speed, dy * speed);

        let (mut px, mut py) = state.pos;
        let (mut rx, mut ry) = state.rem;

        // 속도가 0일 때 누적기 초기화 (입력 해제 시 깔끔한 정지)
        if vx == 0.0 && vy == 0.0 {
            rx = 0.0;
            ry = 0.0;
        }

        // 잔차 누적 (Accumulator)
        rx += vx;
        ry += vy;

        let mut move_x = 0;
        let mut move_y = 0;

        let is_diagonal = vx.abs() > 0.0 && vy.abs() > 0.0;

        if is_diagonal {
            // 대각선 이동 중일 때는 둘 다 1.0 이상일 때만 동시에 갱신하여 지그재그 방지
            if rx.abs() >= 1.0 && ry.abs() >= 1.0 {
                move_x = rx.trunc() as i32;
                move_y = ry.trunc() as i32;
            }
        } else {
            // 직선 이동 중일 때는 각각 독립적으로 갱신
            if rx.abs() >= 1.0 {
                move_x = rx.trunc() as i32;
            }
            if ry.abs() >= 1.0 {
                move_y = ry.trunc() as i32;
            }
        }

        // 추출한 정수만큼 잔차 차감 및 실제 위치 업데이트
        rx -= move_x as f32;
        ry -= move_y as f32;
        px += move_x;
        py += move_y;

        // 경계 체크
        let (max_x, max_y) = state.bounds;
        let clamped_x = px.clamp(0, max_x);
        if px != clamped_x {
            px = clamped_x;
            rx = 0.0;
        }
        let clamped_y = py.clamp(0, max_y);
        if py != clamped_y {
            py = clamped_y;
            ry = 0.0;
        }

        state.pos = (px, py);
        state.rem = (rx, ry);
        state.vel = (vx, vy);
    }
}
