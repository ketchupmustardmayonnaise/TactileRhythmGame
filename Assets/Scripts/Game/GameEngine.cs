using System;
using System.Collections.Generic;
using UnityEngine;

/// <summary>
/// 게임 핵심 로직. AudioManager와 BrailleCellDisplay를 연결해서
/// 노트 스폰 → 이동 → 판정 → 렌더링 루프를 처리한다.
///
/// Classic / Easy는 2key, Single은 1key. 모드별 차이는 RhythmTestModes에서 정의한다.
///
/// 판정:
///   perfect / good 외의 모든 결과는 miss 로 집계된다.
///   - 노트를 windowGood 밖에서 치면        → miss
///   - 노트를 안 치고 흘려보내면(타임아웃)   → miss
///   판정이 확정될 때마다 OnJudge(HitResult) 이벤트가 발생하므로
///   GameScreen 이 이를 구독해 점수 / 개수 / 콤보를 집계한다.
///
/// 타이밍 조정(예고 오프셋):
///   previewOffset(초) 만큼 노트의 "예고(채워짐)"가 실제 판정 시각보다 일찍/늦게
///   완료되도록 시각을 앞당긴다. 판정 자체는 원래 노트 시각을 그대로 쓰므로,
///   플레이어가 화면(채워짐)에 맞춰 치면 자신의 반응 지연이 자동 보정된다.
///   이 값은 '타이밍 조정' 메뉴에서 자동 측정되어 PlayerPrefs 에 저장된다.
/// </summary>
[ExecuteAlways]
public class GameEngine : MonoBehaviour
{
    public const string PrefKeyPreviewOffset = "rg_preview_offset";

    [Header("Test Mode")]
    [Tooltip("Classic: 기존 2key / Easy: 쉬운 2key·좁은 배치 / Single: Space 1key. 실행 중 변경하면 메뉴로 돌아갑니다.")]
    public RhythmTestMode testMode = RhythmTestModes.Default;

    [Header("References")]
    public BrailleCellDisplay display;
    [Tooltip("비워둬도 됨. 실제로는 살아있는 AudioManager 싱글턴을 우선 사용한다.")]
    public AudioManager audioManager;

    /// <summary>
    /// 실제로 사용할 AudioManager.
    /// 씬을 리로드해 메뉴로 돌아오면, 인스펙터의 audioManager 참조는
    /// '새 씬에서 생성됐다가 Awake에서 스스로 파괴되는 중복 인스턴스'를
    /// 가리킬 수 있다(그 인스턴스는 AudioSource(src)가 null 이라 NRE 발생).
    /// 따라서 DontDestroyOnLoad로 살아남은 싱글턴(Instance)을 항상 우선 사용한다.
    /// </summary>
    private AudioManager Audio =>
        AudioManager.Instance != null ? AudioManager.Instance : audioManager;

    [Header("Timing Windows (초)")]
    public float windowPerfect = 0.07f;
    public float windowGood = 0.14f;

    [Tooltip("체크하면 채보의 seconds_per_beat를 무시하고 아래 previewWindow 값을 인스펙터에서 그대로 사용")]
    public bool overridePreviewWindow = true;
    [Tooltip("노트 예고 시간(초): 노트가 다가오며 버튼 내부가 0→1 채워지는 시간. " +
             "Override가 체크돼 있으면 이 값이 그대로 적용되고, 아니면 곡 로드 시 채보 값으로 덮어써짐")]
    public float previewWindow = 0.3f;

    [Tooltip("예고 오프셋(초). '타이밍 조정'에서 자동 설정됨. " +
             "양수 = 예고(채워짐)를 그만큼 일찍 완료시켜, 늦게 치는 성향을 보정한다.")]
    public float previewOffset = 0f;

    public enum VibrationMode
    {
        FrequencySweep,   // 주파수 감소: 예고 동안 startFrequency → 0Hz
        Pulse,            // 펄스: 예고 동안 pulseFrequency 로 일정 유지
    }

    [Header("Braille Vibration (점자 진동)")]
    [Tooltip("진동 표현 모드.\n" +
             "FrequencySweep = 예고 시간 동안 주파수가 startFrequency → 0Hz 로 서서히 감소(0Hz = 계속 켜짐 = 타격 타이밍).\n" +
             "Pulse = 예고 시간 동안 pulseFrequency 로 일정하게 on/off 반복.")]
    public VibrationMode vibrationMode = VibrationMode.Pulse;

    [Tooltip("[FrequencySweep] 시작 주파수(Hz). 예고가 시작될 때의 on/off 반복 속도. " +
             "예고가 끝나는 순간 0Hz(= 계속 켜짐)가 된다.")]
    public float startFrequency = 60f;

    [Tooltip("[Pulse] 예고 시간 동안 유지할 주파수(Hz).")]
    public float pulseFrequency = 40f;

    [Range(0f, 1f)]
    [Tooltip("한 주기(cycle) 중 '켜짐(on)'이 차지하는 비율. 0.5 = 절반 켜짐 / 절반 꺼짐.")]
    public float vibrationDutyCycle = 0.5f;

    [Tooltip("[FrequencySweep] 계단식 주파수 감소 간격(초). 0이면 선형(연속) 감소. " +
             "예: 0.1 = 100ms 간격으로 주파수를 계단식으로 낮춤.")]
    public float frequencyStepInterval = 0f;

    [Header("Classic Buttons (다른 모드는 이 크기를 공유)")]
    [Tooltip("Classic 원본 배치. Easy/Single은 원본을 보존하면서 위치·개수만 바꿉니다.")]
    public BrailleCircleButton[] idleButtons;

    // ── 공개 상태 ─────────────────────────────────────────────────────────────
    public int Score { get; private set; }
    public int Combo { get; private set; }
    public int MaxCombo { get; private set; }   // 가장 길게 이어진 콤보
    public bool IsRunning { get; private set; }
    public bool AutoPlayEnabled { get; private set; }
    public bool VisualVersion { get; private set; }
    // 보정은 사람의 반응 시간을 측정하므로 오토 설정을 잠시 적용하지 않는다.
    public bool IsAutoPlayActive => AutoPlayEnabled && !calibrationSession;

    public int LaneCount => RhythmTestModes.LaneCount(testMode);
    public string OffsetPreferenceKey => RhythmTestModes.OffsetPreferenceKey(testMode);

    /// <summary>판정이 확정될 때마다(Perfect/Good/Miss) 발생.</summary>
    public event Action<HitResult> OnJudge;
    /// <summary>모든 노트가 끝나 곡이 종료되면 1회 발생.</summary>
    public event Action OnFinished;

    private SongData song;
    private int nextNoteIdx;
    private readonly List<ActiveNote> activeNotes = new();
    private float[] laneFlash;
    private float[] lanePhase;   // 레인별 진동 위상 누적기(단위: cycles)
    private int[] laneInputFrame;
    private float lastNoteTime;
    private bool finishedFired;
    private bool calibrationSession;
    public const float CalibrationWindow = 0.3f;
    private BrailleCircleButton[] renderButtons;
    private RhythmTestMode layoutMode;

    // ── Unity 생명주기 ────────────────────────────────────────────────────────

    void Awake()
    {
        EnsureLayout();

        // 저장된 예고 오프셋 불러오기 ('타이밍 조정' 결과)
        if (Application.isPlaying)
            previewOffset = PlayerPrefs.GetFloat(OffsetPreferenceKey, previewOffset);

        ResetLaneState();
        if (display != null)
            display.buttons = renderButtons;
    }

    void Update()
    {
        // 실행 중 모드 변경은 GameScreen이 메뉴 전환과 함께 적용한다.
        // 그 사이 한 프레임에 다른 레인의 노트를 처리하지 않는다.
        if (Application.isPlaying && renderButtons != null && layoutMode != testMode) return;
        EnsureLayout();
        if (display == null) return;

        // 에디터 미리보기 / 대기 화면
        if (!Application.isPlaying || !IsRunning || song == null)
        {
            RenderIdleScreen();
            return;
        }

        float now = (float)Audio.SongTime;
        SpawnNotes(now);
        ProcessAutoPlay(now);
        PruneMissedNotes(now);
        RenderFrame(now);
        CheckFinished(now);
    }

    // ── 공개 API ─────────────────────────────────────────────────────────────

    public void SetAutoPlay(bool enabled)
    {
        if (IsRunning) return;
        AutoPlayEnabled = enabled;
    }

    /// <summary>GameScreen의 공통 모드 전환 지점에서 적용한다.</summary>
    public void SetVisualVersion(bool enabled)
    {
        VisualVersion = enabled;
        if (!enabled && laneFlash != null) Array.Clear(laneFlash, 0, laneFlash.Length);
        if (display != null)
        {
            display.AllowTouchHighlight = enabled && !IsAutoPlayActive;
            if (!enabled) display.ClearHighlights();
        }
    }

    void ResetLaneState()
    {
        laneFlash = new float[LaneCount];
        lanePhase = new float[LaneCount];
        laneInputFrame = new int[LaneCount];
        Array.Fill(laneInputFrame, -1);
    }

    public void LoadSong(SongData songData, bool forCalibration = false)
    {
        EnsureLayout();

        song = songData;
        calibrationSession = forCalibration;
        nextNoteIdx = 0;
        Score = 0;
        Combo = 0;
        MaxCombo = 0;
        IsRunning = false;
        finishedFired = false;
        activeNotes.Clear();
        ResetLaneState();

        // 현재 모드의 레인 밖 노트는 제거해 유령 miss를 방지
        if (song?.notes != null)
        {
            song.notes.RemoveAll(n => n.lane < 0 || n.lane >= LaneCount);
            lastNoteTime = 0f;
            foreach (var n in song.notes)
                if (n.time > lastNoteTime) lastNoteTime = n.time;
        }
        else
        {
            lastNoteTime = 0f;
        }

        // 예고 시간: Override가 꺼져 있을 때만 채보의 seconds_per_beat로 자동 설정.
        if (!overridePreviewWindow && song?.meta != null && song.meta.seconds_per_beat > 0f)
            previewWindow = song.meta.seconds_per_beat;
    }

    /// <summary>채보의 오디오(Resources)를 로드해 시작.</summary>
    public void StartGame(float countdownSeconds = 3f)
    {
        if (song == null) return;
        AudioClip clip = Resources.Load<AudioClip>(song.AudioResourcePath);
        if (clip == null)
            Debug.LogWarning($"[GameEngine] 오디오 파일 없음: {song.AudioResourcePath}");
        StartGameWithClip(clip, countdownSeconds);
    }

    /// <summary>외부에서 만든 클립으로 시작(타이밍 조정용 무음 클립 등).</summary>
    public void StartGameWithClip(AudioClip clip, float countdownSeconds = 3f)
    {
        if (song == null) return;
        if (clip == null)
        {
            IsRunning = false;
            Debug.LogError("[GameEngine] 재생할 오디오가 없어 시작하지 못했습니다.");
            return;
        }
        IsRunning = true;
        finishedFired = false;
        Audio.SchedulePlay(clip, countdownSeconds, 0f);
    }

    /// <summary>진행 즉시 중지(타이밍 조정 종료 등). 화면은 대기 상태로 돌아간다.</summary>
    public void Halt()
    {
        IsRunning = false;
        activeNotes.Clear();
    }

    /// <summary>씬을 다시 로드하지 않고 메뉴/다른 테스트 모드를 준비한다.</summary>
    public void ResetForMenu()
    {
        Halt();
        song = null;
        calibrationSession = false;
        nextNoteIdx = 0;
        Score = Combo = MaxCombo = 0;
        finishedFired = false;
        EnsureLayout();
        ResetLaneState();
        previewOffset = PlayerPrefs.GetFloat(OffsetPreferenceKey, 0f);
    }

    /// <summary>레인(= 버튼 인덱스, 0-based)을 탭했을 때 호출. 판정 결과를 반환.</summary>
    public HitResult TapLane(int lane)
    {
        if (!IsRunning || IsAutoPlayActive || lane < 0 || lane >= LaneCount) return HitResult.None;

        // 판정은 예고 오프셋과 무관하게 '원래 노트 시각' 기준.
        float now = (float)Audio.SongTime;
        // 화면 스크립트가 먼저 Update되어도 현재 예고 중인 노트를 처리한다.
        SpawnNotes(now);
        ActiveNote best = null;
        float bestDiff = float.MaxValue;

        foreach (var n in activeNotes)
        {
            if (n.data.lane != lane || n.isHit) continue;
            float diff = Mathf.Abs(n.data.time - now);
            if (diff < bestDiff) { bestDiff = diff; best = n; }
        }

        // 근처에 노트가 전혀 없으면 헛침(패널티 없음)
        if (best == null) { FinishLaneInput(lane, 0.12f); return HitResult.None; }

        if (bestDiff <= windowPerfect)
        {
            RegisterHit(best, lane, 300);
            Emit(HitResult.Perfect);
            return HitResult.Perfect;
        }
        if (bestDiff <= windowGood)
        {
            RegisterHit(best, lane, 100);
            Emit(HitResult.Good);
            return HitResult.Good;
        }

        // good 밖에서 친 경우: 그 노트를 소비하고 miss 처리
        best.isHit = true;
        Combo = 0;
        FinishLaneInput(lane, 0.12f);
        Emit(HitResult.Miss);
        return HitResult.Miss;
    }

    /// <summary>
    /// 타이밍 조정용: 탭한 레인에서 가장 가까운 노트와의 부호 있는 오차(raw)를 구한다.
    /// 반환 오차 = (탭 시각 - 노트 시각). 양수면 늦게 친 것.
    /// 채점/이벤트를 발생시키지 않으며, 측정된 노트는 소비(중복 방지)한다.
    /// </summary>
    public bool TryMeasureTap(int lane, out float rawError)
    {
        rawError = 0f;
        if (!IsRunning || lane < 0 || lane >= LaneCount) return false;

        float now = (float)Audio.SongTime;
        // GameScreen.Update가 엔진보다 먼저 호출된 프레임에도 보정 후보를 확보한다.
        SpawnNotes(now);
        ActiveNote best = null;
        float bestDiff = float.MaxValue;

        foreach (var n in activeNotes)
        {
            if (n.data.lane != lane || n.isHit) continue;
            float diff = Mathf.Abs(n.data.time - now);
            if (diff < bestDiff) { bestDiff = diff; best = n; }
        }

        // 너무 먼 탭(±0.3s 밖)은 표본에서 제외
        if (best == null || bestDiff > CalibrationWindow)
        {
            if (best != null) best.previewDismissed = true;
            FinishLaneInput(lane, 0.12f);
            return false;
        }

        best.isHit = true;
        FinishLaneInput(lane, 0.15f);
        rawError = now - best.data.time;
        return true;
    }

    // ── 내부: 노트 관리 ──────────────────────────────────────────────────────

    void SpawnNotes(float now)
    {
        float lookAhead = previewWindow + Mathf.Max(0f, previewOffset) + 0.1f;
        if (calibrationSession) lookAhead = Mathf.Max(lookAhead, CalibrationWindow);
        while (nextNoteIdx < song.notes.Count)
        {
            var nd = song.notes[nextNoteIdx];
            if (nd.time <= now + lookAhead) { activeNotes.Add(new ActiveNote(nd)); nextNoteIdx++; }
            else break;
        }
    }

    void RegisterHit(ActiveNote note, int lane, int baseScore)
    {
        note.isHit = true;
        Combo++;
        if (Combo > MaxCombo) MaxCombo = Combo;
        Score += baseScore + Combo * 5;
        FinishLaneInput(lane, 0.15f);
    }

    void FinishLaneInput(int lane, float flashSeconds)
    {
        lanePhase[lane] = 0f;
        laneInputFrame[lane] = Time.frameCount;
        laneFlash[lane] = VisualVersion ? flashSeconds : 0f;
        if (display == null || renderButtons == null) return;
        // 엔진과 입력의 Update 순서와 무관하게 이번 프레임부터 예고를 지운다.
        var button = renderButtons[lane];
        button.SetHighlight(display, false);
        button.DrawFill(display, 0f);
        if (VisualVersion) button.SetHighlight(display, true);
        display.Refresh();
    }

    void ProcessAutoPlay(float now)
    {
        if (!IsRunning || !IsAutoPlayActive) return;
        // 예고 보정과 무관하게 원래 노트 시각에 판정한다.
        // 프레임이 늦거나 동시치기가 있어도 모든 노트를 한 번씩 처리한다.
        foreach (var note in activeNotes)
        {
            if (note.isHit || note.data.time > now) continue;
            RegisterHit(note, note.data.lane, 300);
            Emit(HitResult.Perfect);
        }
    }

    void PruneMissedNotes(float now)
    {
        for (int i = activeNotes.Count - 1; i >= 0; i--)
        {
            var n = activeNotes[i];
            if (n.isHit)
            {
                activeNotes.RemoveAt(i);
            }
            else if (n.data.time < now - (calibrationSession ? CalibrationWindow : windowGood))
            {
                // 안 치고 흘려보냄 → miss
                activeNotes.RemoveAt(i);
                if (!calibrationSession)
                {
                    Combo = 0;
                    Emit(HitResult.Miss);
                }
            }
        }
    }

    void CheckFinished(float now)
    {
        if (finishedFired || !IsRunning || song == null) return;
        if (nextNoteIdx < song.notes.Count) return;   // 아직 남은 노트
        if (activeNotes.Count > 0) return;             // 화면에 노트 남음
        if (now < lastNoteTime + windowGood + 0.3f) return;
        // 마지막 노트와 음악의 끝을 구분한다. 예약 재생 전이나 수동 정지도 완료가 아니다.
        if (!calibrationSession && !Audio.HasPlaybackFinished) return;

        finishedFired = true;
        IsRunning = false;
        OnFinished?.Invoke();
    }

    void Emit(HitResult r) => OnJudge?.Invoke(r);

    // ── 내부: 렌더 ──────────────────────────────────────────────────────────

    void RenderIdleScreen()
    {
        if (renderButtons == null) return;
        display.ClearAll();
        foreach (var btn in renderButtons)
            btn.Draw(display);
        display.Refresh();
    }

    void RenderFrame(float now)
    {
        display.ClearAll();

        // 1. 레인별 '가장 임박한 노트'의 남은 시간(timeLeft)을 구한다.
        //    previewOffset 만큼 '완료 시각'을 앞당겨(늦춰) 예고한다.
        var laneTimeLeft = new float[LaneCount];
        var laneHasNote = new bool[LaneCount];
        for (int i = 0; i < LaneCount; i++) laneTimeLeft[i] = float.MaxValue;

        foreach (var n in activeNotes)
        {
            int lane = n.data.lane;
            if (lane < 0 || lane >= LaneCount || n.isHit || n.previewDismissed) continue;

            float timeLeft = (n.data.time - previewOffset) - now;
            if (timeLeft > previewWindow) continue;          // 아직 예고 구간에 진입 전
            if (timeLeft < laneTimeLeft[lane])               // 가장 임박한(또는 막 지난) 노트
            {
                laneTimeLeft[lane] = timeLeft;
                laneHasNote[lane] = true;
            }
        }

        // 2. 버튼 렌더 — 테두리는 항상 표시, 내부는 진동(on/off) 상태.
        //    FrequencySweep: 주파수 startFrequency→0 으로 감소, 0Hz(=timeLeft<=0)에서 계속 켜짐.
        //    Pulse         : pulseFrequency 로 일정하게 on/off, timeLeft<=0 에서 계속 켜짐.
        //    히트 시에는 파란색 플래시.
        for (int lane = 0; lane < LaneCount; lane++)
        {
            if (VisualVersion && laneFlash[lane] > 0f)
            {
                laneFlash[lane] -= Time.deltaTime;
                renderButtons[lane].Draw(display);
                renderButtons[lane].SetHighlight(display, true);
                lanePhase[lane] = 0f;
                continue;
            }

            if (laneInputFrame[lane] == Time.frameCount || !laneHasNote[lane])
            {
                // 활성 노트 없음 → 내부 꺼짐(테두리만 유지)
                renderButtons[lane].DrawFill(display, 0f);
                lanePhase[lane] = 0f;
                continue;
            }

            float timeLeft = laneTimeLeft[lane];
            float interior;
            if (timeLeft <= 0f)
            {
                // 타격 타이밍 도달: 주파수 0 → 계속 켜짐(on)
                interior = 1f;
                lanePhase[lane] = 0f;
            }
            else
            {
                // 시간에 따라 변하는 주파수를 위상에 누적(프레임 레이트와 무관하게 정확)
                float freq = CurrentFrequency(timeLeft);
                lanePhase[lane] += freq * Time.deltaTime;
                float frac = lanePhase[lane] - Mathf.Floor(lanePhase[lane]);
                interior = (frac < vibrationDutyCycle) ? 1f : 0f;
            }

            renderButtons[lane].DrawFill(display, interior);
        }

        display.Refresh();
    }

    /// <summary>
    /// 현재 남은 시간(timeLeft, 초)에 대응하는 진동 주파수(Hz)를 계산한다.
    ///   Pulse          : 항상 pulseFrequency.
    ///   FrequencySweep : timeLeft==previewWindow 에서 startFrequency,
    ///                    timeLeft==0 에서 0Hz 로 (선형 또는 계단식) 감소.
    /// </summary>
    float CurrentFrequency(float timeLeft)
    {
        if (vibrationMode == VibrationMode.Pulse)
            return Mathf.Max(0f, pulseFrequency);

        float win = Mathf.Max(0.0001f, previewWindow);

        // 계단식: timeLeft 를 step 간격으로 양자화해 주파수를 계단처럼 떨어뜨린다.
        if (frequencyStepInterval > 0f)
        {
            float steppedTimeLeft = Mathf.Ceil(timeLeft / frequencyStepInterval) * frequencyStepInterval;
            return Mathf.Max(0f, startFrequency * Mathf.Clamp01(steppedTimeLeft / win));
        }

        // 선형: startFrequency → 0
        return Mathf.Max(0f, startFrequency * Mathf.Clamp01(timeLeft / win));
    }

    // ── 모드별 배치 (Classic의 직렬화된 배치는 보존) ─────────────────────────

    void EnsureLayout()
    {
        if (idleButtons == null || idleButtons.Length != 2
            || idleButtons[0] == null || idleButtons[1] == null)
            idleButtons = TwoKeyButtons();
        if (!Application.isPlaying || renderButtons == null || layoutMode != testMode)
        {
            renderButtons = RhythmTestModes.CreateLayout(testMode, idleButtons);
            layoutMode = testMode;
        }
        if (display != null)
        {
            display.buttons = renderButtons;
            display.AllowTouchHighlight = VisualVersion && !IsAutoPlayActive;
        }
    }

    // 2key: 좌/우 중앙 (← 또는 A / → 또는 D)
    static BrailleCircleButton[] TwoKeyButtons()
    {
        return new BrailleCircleButton[]
        {
            new BrailleCircleButton(0.5f, 0.25f, 0.14f, 0.266f),   // Lane 0: 왼쪽
            new BrailleCircleButton(0.5f, 0.75f, 0.14f, 0.266f),   // Lane 1: 오른쪽
        };
    }
}
