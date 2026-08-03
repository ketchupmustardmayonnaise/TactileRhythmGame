using System;
using System.Collections.Generic;
using UnityEngine;

/// <summary>
/// 게임 핵심 로직. AudioManager와 BrailleCellDisplay를 연결해서
/// 노트 스폰 → 이동 → 판정 → 렌더링 루프를 처리한다.
///
/// 이 버전은 2key 모드 전용이다. (레인 2개: 왼쪽 D / 오른쪽 K)
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
    public bool overridePreviewWindow = false;
    [Tooltip("노트 예고 시간(초): 노트가 다가오며 버튼 내부가 0→1 채워지는 시간. " +
             "Override가 체크돼 있으면 이 값이 그대로 적용되고, 아니면 곡 로드 시 채보 값으로 덮어써짐")]
    public float previewWindow = 0.5f;

    [Tooltip("예고 오프셋(초). '타이밍 조정'에서 자동 설정됨. " +
             "양수 = 예고(채워짐)를 그만큼 일찍 완료시켜, 늦게 치는 성향을 보정한다.")]
    public float previewOffset = 0f;

    [Header("Idle Screen Buttons  (왼쪽 = Lane0 / 오른쪽 = Lane1)")]
    [Tooltip("비워두거나 2개가 아니면 코드가 자동으로 2key 배치를 넣는다.")]
    public BrailleCircleButton[] idleButtons;

    // ── 공개 상태 ─────────────────────────────────────────────────────────────
    public int Score { get; private set; }
    public int Combo { get; private set; }
    public int MaxCombo { get; private set; }   // 가장 길게 이어진 콤보
    public bool IsRunning { get; private set; }

    /// <summary>레인 수 = 버튼 수 (2key 이므로 항상 2)</summary>
    public int LaneCount => idleButtons != null ? idleButtons.Length : 0;

    /// <summary>판정이 확정될 때마다(Perfect/Good/Miss) 발생.</summary>
    public event Action<HitResult> OnJudge;
    /// <summary>모든 노트가 끝나 곡이 종료되면 1회 발생.</summary>
    public event Action OnFinished;

    private SongData song;
    private int nextNoteIdx;
    private readonly List<ActiveNote> activeNotes = new();
    private float[] laneFlash;
    private float lastNoteTime;
    private bool finishedFired;

    // ── Unity 생명주기 ────────────────────────────────────────────────────────

    void Awake()
    {
        EnsureTwoKeyLayout();

        // 저장된 예고 오프셋 불러오기 ('타이밍 조정' 결과)
        if (Application.isPlaying)
            previewOffset = PlayerPrefs.GetFloat(PrefKeyPreviewOffset, previewOffset);

        laneFlash = new float[LaneCount];
        if (display != null)
            display.buttons = idleButtons;
    }

    void Update()
    {
        if (display == null) return;

        // 에디터 미리보기 / 대기 화면
        if (!Application.isPlaying || !IsRunning || song == null)
        {
            RenderIdleScreen();
            return;
        }

        float now = (float)Audio.SongTime;
        SpawnNotes(now);
        PruneMissedNotes(now);
        RenderFrame(now);
        CheckFinished(now);
    }

    // ── 공개 API ─────────────────────────────────────────────────────────────

    public void LoadSong(SongData songData)
    {
        EnsureTwoKeyLayout();

        song = songData;
        nextNoteIdx = 0;
        Score = 0;
        Combo = 0;
        MaxCombo = 0;
        IsRunning = false;
        finishedFired = false;
        activeNotes.Clear();
        laneFlash = new float[LaneCount];

        // 2key 밖(레인 인덱스 >= 2)의 노트는 제거해 유령 miss를 방지
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
        IsRunning = true;
        finishedFired = false;
        if (clip != null)
            Audio.SchedulePlay(clip, countdownSeconds, 0f);
    }

    /// <summary>진행 즉시 중지(타이밍 조정 종료 등). 화면은 대기 상태로 돌아간다.</summary>
    public void Halt()
    {
        IsRunning = false;
        activeNotes.Clear();
    }

    /// <summary>레인(= 버튼 인덱스, 0-based)을 탭했을 때 호출. 판정 결과를 반환.</summary>
    public HitResult TapLane(int lane)
    {
        if (!IsRunning || lane < 0 || lane >= LaneCount) return HitResult.None;

        // 판정은 예고 오프셋과 무관하게 '원래 노트 시각' 기준.
        float now = (float)Audio.SongTime;
        ActiveNote best = null;
        float bestDiff = float.MaxValue;

        foreach (var n in activeNotes)
        {
            if (n.data.lane != lane || n.isHit) continue;
            float diff = Mathf.Abs(n.data.time - now);
            if (diff < bestDiff) { bestDiff = diff; best = n; }
        }

        // 근처에 노트가 전혀 없으면 헛침(패널티 없음)
        if (best == null) { laneFlash[lane] = 0.12f; return HitResult.None; }

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
        laneFlash[lane] = 0.12f;
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
        ActiveNote best = null;
        float bestDiff = float.MaxValue;

        foreach (var n in activeNotes)
        {
            if (n.data.lane != lane || n.isHit) continue;
            float diff = Mathf.Abs(n.data.time - now);
            if (diff < bestDiff) { bestDiff = diff; best = n; }
        }

        // 너무 먼 탭(±0.3s 밖)은 표본에서 제외
        if (best == null || bestDiff > 0.3f) { laneFlash[lane] = 0.12f; return false; }

        best.isHit = true;
        laneFlash[lane] = 0.15f;
        rawError = now - best.data.time;
        return true;
    }

    // ── 내부: 노트 관리 ──────────────────────────────────────────────────────

    void SpawnNotes(float now)
    {
        float lookAhead = previewWindow + Mathf.Max(0f, previewOffset) + 0.1f;
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
        laneFlash[lane] = 0.15f;
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
            else if (n.data.time < now - windowGood)
            {
                // 안 치고 흘려보냄 → miss
                Combo = 0;
                activeNotes.RemoveAt(i);
                Emit(HitResult.Miss);
            }
        }
    }

    void CheckFinished(float now)
    {
        if (finishedFired) return;
        if (nextNoteIdx < song.notes.Count) return;   // 아직 남은 노트
        if (activeNotes.Count > 0) return;             // 화면에 노트 남음
        if (now < lastNoteTime + windowGood + 0.3f) return;

        finishedFired = true;
        IsRunning = false;
        OnFinished?.Invoke();
    }

    void Emit(HitResult r) => OnJudge?.Invoke(r);

    // ── 내부: 렌더 ──────────────────────────────────────────────────────────

    void RenderIdleScreen()
    {
        if (idleButtons == null) return;
        display.ClearAll();
        foreach (var btn in idleButtons)
            btn.Draw(display);
        display.Refresh();
    }

    void RenderFrame(float now)
    {
        display.ClearAll();

        // 1. 레인별 예고 activation 계산.
        //    previewOffset 만큼 '완료 시각'을 앞당겨(늦춰) 예고한다.
        var activations = new float[LaneCount];
        foreach (var n in activeNotes)
        {
            int lane = n.data.lane;
            if (lane < 0 || lane >= LaneCount || n.isHit) continue;

            float timeLeft = (n.data.time - previewOffset) - now;
            if (timeLeft >= 0f && timeLeft <= previewWindow)
                activations[lane] = Mathf.Max(activations[lane], 1f - timeLeft / previewWindow);
        }

        // 2. 버튼 렌더 — 테두리는 항상 표시, 내부만 0→1 점진, 히트 시 파란색 플래시
        for (int lane = 0; lane < LaneCount; lane++)
        {
            if (laneFlash[lane] > 0f)
            {
                laneFlash[lane] -= Time.deltaTime;
                idleButtons[lane].Draw(display);
                idleButtons[lane].SetHighlight(display, true);
            }
            else
            {
                idleButtons[lane].DrawFill(display, activations[lane]);
            }
        }

        display.Refresh();
    }

    // ── 2key 배치 ────────────────────────────────────────────────────────────
    //   인스펙터 idleButtons 가 정확히 2개면 그대로 쓰고, 아니면 기본 배치로 강제.

    void EnsureTwoKeyLayout()
    {
        if (idleButtons == null || idleButtons.Length != 2)
            idleButtons = TwoKeyButtons();
    }

    // 2key: 좌/우 중앙 (D / K)
    static BrailleCircleButton[] TwoKeyButtons()
    {
        return new BrailleCircleButton[]
        {
            new BrailleCircleButton(0.5f, 0.25f, 0.14f, 0.266f),   // Lane 0: 왼쪽  (D)
            new BrailleCircleButton(0.5f, 0.75f, 0.14f, 0.266f),   // Lane 1: 오른쪽 (K)
        };
    }
}
