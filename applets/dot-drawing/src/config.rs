use physics::PhysicsConfig;

/// 뷰포트 설정 구조체
#[derive(Clone, Copy, Debug)]
pub struct ViewConfig {
    pub canvas_border_width: i16, // 캔버스 테두리 두께
}
impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            canvas_border_width: 3,
        }
    }
}
/// 시스템 설정 구조체
#[derive(Clone, Copy, Debug)]
pub struct SystemConfig {
    pub target_fps: u64,
    // 목표 프레임률
    pub max_accumulator_ms: u64,
    // 최대 누적 시간 (밀리초)
    pub max_dir_key_history: usize, // 방향키 입력 기록 최대 길이
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            target_fps: 60,
            max_accumulator_ms: 200,
            max_dir_key_history: 2,
        }
    }
}

/// 전체 게임 설정 구조체
#[derive(Clone, Copy, Debug)]
pub struct GameConfig {
    pub physics: PhysicsConfig,
    pub view: ViewConfig,
    pub system: SystemConfig,
}

impl Default for GameConfig {
    fn default() -> Self {
        const PHYSICS_ACCEL_FORCE: i32 = 160; // 가속력
        const PHYSICS_DIAGONAL_ACCEL: i32 = 113; // 대각선 가속력 (가속력 / sqrt(2))
        const PHYSICS_ACCEL_SCALE: f32 = 1.0 / 2048.0; // 가속력 스케일 (입력값을 실제 가속력으로 변환하는 데 사용)
        const PHYSICS_FRICTION_COEFF: f32 = 180.0 / 256.0; // 마찰 계수 (속도에 곱해져서 감속 효과를 냄)
        const PHYSICS_VELOCITY_EPSILON: f32 = 0.001; // 속도 0으로 간주하는 임계값 (이 값보다 작은 속도는 0으로 처리)
        const PHYSICS_STANDARD_SPEED: f32 = 0.5; // 표준 이동 속도 (최대 가속력으로 이동할 때의 속도)

        Self {
            physics: PhysicsConfig {
                accel_force: PHYSICS_ACCEL_FORCE,
                diagonal_accel: PHYSICS_DIAGONAL_ACCEL,
                accel_scale: PHYSICS_ACCEL_SCALE,
                friction_coeff: PHYSICS_FRICTION_COEFF,
                velocity_epsilon: PHYSICS_VELOCITY_EPSILON,
                standard_speed: PHYSICS_STANDARD_SPEED,
            },
            view: ViewConfig::default(),
            system: SystemConfig::default(),
        }
    }
}
