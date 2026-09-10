# 비교 테스트

Unity Hierarchy의 **GameManager → Game Engine → Test Mode**에서 선택합니다.

| 모드 | 채보 | 입력 | 표시 |
| --- | --- | --- | --- |
| Classic | 기존 `_2k.json` | ←/→ 또는 A/D | 기존 두 원 |
| Easy | `_easy.json` | ←/→ 또는 A/D | 중앙으로 모은 두 원 |
| Single | `_single.json` | Space | 중앙 원 하나 |

- 현재 비교 기준: **Pulse, Pulse Frequency 40, Override Preview Window 켜짐, Preview Window 0.3**. 모드 전환은 이 진동·판정 설정을 바꾸지 않습니다.
- Play 전 선택하면 씬에 저장할 수 있습니다. Play 중 변경하면 진행을 중단하고 메뉴로 돌아갑니다. Esc·결과 화면 복귀에도 선택을 유지합니다. Play 종료 시 변경이 되돌아가는 것은 Unity 기본 동작입니다.
- 곡은 기존 **Game Screen → Song Resource Name**에서 접미사 없이 선택합니다.
- 시작 메뉴에서 **F2**로 **AUTO ON/OFF**를 전환합니다(실행 시작 시 OFF). ON이면 우상단에 작은 흰 삼각형이 표시되며, 콘솔 출력 모드에서도 보입니다. 모든 모드에서 노트 시각에 자동 Perfect 판정·효과음·점수·콤보가 적용됩니다. 노트 키와 마우스/터치 하이라이트는 무시하며 **Esc 메뉴 복귀**는 유지합니다. 메뉴 복귀나 테스트 모드 변경 후에도 오토 선택을 유지합니다.
- 타이밍 조정 중에는 오토와 삼각형 표시를 잠시 끄고 직접 입력을 측정합니다. 메뉴 복귀 시 오토 선택을 다시 적용합니다.
- 점 하나의 지름은 기존보다 10% 크게 표시합니다(`Braille Cell Display → Dot Fill Ratio`: 0.7 → 0.77). 원의 배치와 점 간격은 유지합니다.
- C 보정도 각 모드의 입력을 사용합니다. Easy/Single 보정 노트는 1.25초 간격이며 보정값은 모드별로 저장됩니다.
- Single 보정: Game 화면에 키보드 포커스를 두고 메뉴에서 **C → Space**로 16개 표본을 입력합니다. 보정은 ±0.3초 범위에서 측정하며, 일반 게임의 판정 폭과 별도로 처리합니다.
- **GameManager → Game Screen → Visual Version**: 기본값 **False**에서는 텍스트·메뉴/결과 패널과 파란 타격 효과를 숨기고 Unity Console에 안내·점수·판정·보정·결과를 출력합니다. **True**이면 기존 텍스트 UI와 파란 효과를 표시합니다. 노트 예고와 Auto 삼각형은 양쪽 모두 유지되며 실행 중에도 전환할 수 있습니다. 기존 `Console Text Only`와 값의 의미가 반대입니다. 콘솔을 클릭하면 게임이 키 입력을 받지 못하므로 조작 전 Game 화면을 다시 클릭하세요.
- 키를 입력하면 해당 노트의 예고가 즉시 꺼집니다. 일반 플레이와 타이밍 조정 모두 적용하며, 다음 노트의 예고는 정상적으로 이어집니다. Visual Version=False에서는 키 입력이나 마우스/터치로 파란색이 나타나지 않습니다.
- 결과는 마지막 노트 처리와 **음악 재생이 모두 끝난 뒤** 표시됩니다. 콘솔 모드에서도 결과 안내 후 아무 키로 메뉴에 돌아갑니다.
- 기존 채보·음원·판정 폭·효과음은 공유/보존합니다. Capture를 통한 실제 촉각 주파수는 화면 갱신과 장치에 영향을 받으므로 40Hz 전달을 보장하지 않습니다.

## 채보 다시 생성

프로젝트 루트에서 Python 3으로 실행합니다. 추가 패키지는 필요 없습니다.

```powershell
python Python/chart_generator_lite_grid.py --test-modes
# 더 느리게 시험하려면 (하한 1초)
python Python/chart_generator_lite_grid.py --test-modes --min-gap 1.5
```

`Assets/Resources/Songs/*_2k.json`의 타이밍을 솎아 같은 폴더에 Easy/Single을 생성합니다. 기본 최소 간격은 **1.25초**이고, 동시치기는 제거합니다. 두 모드는 같은 타이밍을 사용합니다. 기존 `_2k.json`은 덮어쓰지 않습니다. 음원에서 새 원본을 생성하는 기존 옵션도 그대로 사용할 수 있습니다.

## 유지보수 / 검증

모드별 접미사·입력·레인 수·배치·보정 설정은 `Assets/Scripts/Game/RhythmTestMode.cs`, 파생 채보 생성은 `Python/chart_test_modes.py`에 모았습니다. 나중에 모드를 제거할 때 해당 정의와 대응 채보만 정리하고 공통 판정·오디오·결과 코드는 유지할 수 있습니다. 삭제할 enum을 씬이 참조한다면 먼저 남길 모드로 바꾸어 저장하세요.

```powershell
python -m unittest discover -s Python -p test_chart_test_modes.py
```
