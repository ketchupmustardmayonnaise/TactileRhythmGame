/// 시스템 레벨의 메시지 타입입니다.
#[derive(Debug, Clone, PartialEq)]
pub enum SystemMessageType {
    /// 배터리 잔량 변경 (0~100)
    BatteryLevelChanged(u8),
    /// 블루투스/네트워크 연결 상태 변경
    ConnectionStateChanged(bool),
    /// USB 전원 연결/해제
    PowerPlugged(bool),
}
