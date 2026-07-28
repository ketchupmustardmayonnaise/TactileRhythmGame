using System.Collections.Generic;
using UnityEngine;

/// <summary>
/// 게임 핵심 로직. AudioManager와 BrailleCellDisplay를 연결해서
/// 노트 스폰 → 이동 → 판정 → 렌더링 루프를 처리한다.
/// 레인 수 = idleButtons 배열 길이 (2 / 4 / 6). keyMode로 배치를 전환한다.
/// 6key는 인스펙터에 설정된 원본 배치를 그대로 사용한다(코드로 덮어쓰지 않음).
/// </summary>
[ExecuteAlways]
public class GameEngine : MonoBehaviour
{
    [Header("References")]
    public BrailleCellDisplay display;
    public AudioManager audioManager;

    [Header("Timing Windows (초)")]
    public float windowPerfect = 0.07f;
    public float windowGood = 0.14f;
    [Tooltip("체크하면 채보의 seconds_per_beat를 무시하고 아래 previewWindow 값을 인스펙터에서 그대로 사용")]
    public bool overridePreviewWindow = false;
    [Tooltip("노트 예고 시간(초): 노트가 다가오며 버튼 activation이 0→1 되는 시간. " +
             "Override가 체크돼 있으면 이 값이 그대로 적용되고, 아니면 곡 로드 시 채보 값으로 덮어써짐")]
    public float previewWindow = 0.5f;

    [Header("Key Mode")]
    [Tooltip("레인 수(2/4/6). 메뉴에서 SetKeyMode로 설정됨. 인스펙터 값은 에디터 미리보기용")]
    public int keyMode = 6;

    [Header("Idle Screen Buttons  (왼쪽 1-3, 오른쪽 4-6)")]
    public BrailleCircleButton[] idleButtons;

    // 공개 상태
    public int Score { get; private set; }
    public int Combo { get; private set; }
    public bool IsRunning { get; private set; }

    /// <summary>레인 수 = 버튼 수</summary>
    public int LaneCount => idleButtons != null ? idleButtons.Length : 0;

    private SongData song;
    private int nextNoteIdx;
    private readonly List<ActiveNote> activeNotes = new();
    private float[] laneFlash;

    /// <summary>인스펙터/씬에 설정된 원본 6key 배치 보존 (6key 복원용)</summary>
    private BrailleCircleButton[] sixKeyButtons;

    // ── Unity 생명주기 ────────────────────────────────────────────────────────

    void Awake()
    {
        if (idleButtons == null || idleButtons.Length == 0)
            idleButtons = DefaultButtons();

        sixKeyButtons = idleButtons;      // 원본 6key 배치 보존 (SetKeyMode(6)에서 복원)
        laneFlash = new float[LaneCount];

        if (display != null)
            display.buttons = idleButtons;
    }

    void Update()
    {
        if (display == null) return;

        if (!Application.isPlaying)
        {
            RenderIdleScreen();
            return;
        }

        if (!IsRunning || song == null)
        {
            RenderIdleScreen();
            return;
        }

        float now = (float)audioManager.SongTime;
        SpawnNotes(now);
        PruneMissedNotes(now);
        RenderFrame(now);
    }

    // ── 공개 API ─────────────────────────────────────────────────────────────

    /// <summary>
    /// 레인 모드(2/4/6) 전환.
    /// 2/4key는 코드 배치를 사용하고, 6key는 인스펙터 원본을 그대로 복원한다.
    /// </summary>
    public void SetKeyMode(int mode)
    {
        keyMode = mode;
        idleButtons = mode == 2 ? TwoKeyButtons()
                    : mode == 4 ? FourKeyButtons()
                    : (sixKeyButtons ?? DefaultButtons());   // 6key: 원본 유지

        laneFlash = new float[idleButtons.Length];

        if (display != null)
            display.buttons = idleButtons;
    }

    public void LoadSong(SongData songData)
    {
        song = songData;
        nextNoteIdx = 0;
        Score = 0;
        Combo = 0;
        IsRunning = false;
        activeNotes.Clear();
        laneFlash = new float[LaneCount];

        // 예고 시간: Override가 꺼져 있을 때만 채보의 seconds_per_beat로 자동 설정.
        // (Override 체크 시에는 인스펙터의 previewWindow 값을 그대로 유지)
        if (!overridePreviewWindow && song.meta != null && song.meta.seconds_per_beat > 0f)
        {
            previewWindow = song.meta.seconds_per_beat;
        }
    }

    public void StartGame(float countdownSeconds = 3f)
    {
        if (song == null) return;
        IsRunning = true;

        AudioClip clip = Resources.Load<AudioClip>(song.AudioResourcePath);
        if (clip != null)
            audioManager.SchedulePlay(clip, countdownSeconds, 0f);
        else
            Debug.LogWarning($"[GameEngine] 오디오 파일 없음: {song.AudioResourcePath}");
    }

    /// <summary>레인(= 버튼 인덱스, 0-based)을 탭했을 때 호출.</summary>
    public HitResult TapLane(int lane)
    {
        if (!IsRunning || lane < 0 || lane >= LaneCount) return HitResult.None;

        float now = (float)audioManager.SongTime;
        ActiveNote best = null;
        float bestDiff = float.MaxValue;

        foreach (var n in activeNotes)
        {
            if (n.data.lane != lane || n.isHit) continue;
            float diff = Mathf.Abs(n.data.time - now);
            if (diff < bestDiff) { bestDiff = diff; best = n; }
        }

        if (best == null) { laneFlash[lane] = 0.12f; return HitResult.None; }
        if (bestDiff <= windowPerfect) { RegisterHit(best, lane, 300); return HitResult.Perfect; }
        if (bestDiff <= windowGood) { RegisterHit(best, lane, 100); return HitResult.Good; }

        laneFlash[lane] = 0.12f;
        return HitResult.None;
    }

    // ── 내부: 노트 관리 ──────────────────────────────────────────────────────

    void SpawnNotes(float now)
    {
        float lookAhead = previewWindow + 0.1f;
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
        Score += baseScore + Combo * 5;
        laneFlash[lane] = 0.15f;
    }


    void PruneMissedNotes(float now)
    {
        for (int i = activeNotes.Count - 1; i >= 0; i--)
        {
            var n = activeNotes[i];
            if (n.isHit || n.data.time < now - windowGood)
            {
                if (!n.isHit) Combo = 0;
                activeNotes.RemoveAt(i);
            }
        }
    }

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

        // 1. 레인별 노트 예고 activation 계산
        var activations = new float[LaneCount];
        foreach (var n in activeNotes)
        {
            int lane = n.data.lane;
            if (lane < 0 || lane >= LaneCount || n.isHit) continue;
            float timeLeft = n.data.time - now;

            // 쳐야 하는 노트이며 & 남은 시간이 previewWindow 보다 적으면
            if (timeLeft >= 0f && timeLeft <= previewWindow)
                activations[lane] = Mathf.Max(activations[lane], 1f - timeLeft / previewWindow);
        }

        // 2. 버튼 렌더 — 테두리는 항상 active, 내부만 0→1 점진, 히트 시 파란색
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

    // ── 모드별 버튼 배치 ─────────────────────────────────────────────────────
    //   BrailleCircleButton(rowRatio, colRatio, radiusRatio, thicknessRatio)
    //   rowRatio: 0=위, 1=아래 (세로)  /  colRatio: 0=왼쪽, 1=오른쪽 (가로)
    //   thicknessRatio는 원본 6key 링과 같은 절대 두께(≈0.6 dot, 한 줄)가 되도록 지정.

    // 2key: 좌/우 중앙 (D / K)
    static BrailleCircleButton[] TwoKeyButtons()
    {
        return new BrailleCircleButton[]
        {
            new BrailleCircleButton(0.5f, 0.25f, 0.14f, 0.266f),   // Lane 0: 왼쪽  (D)
            new BrailleCircleButton(0.5f, 0.75f, 0.14f, 0.266f),   // Lane 1: 오른쪽 (K)
        };
    }

    // 4key: 좌열(위/아래) + 우열(위/아래) (W S / I K)
    static BrailleCircleButton[] FourKeyButtons()
    {
        return new BrailleCircleButton[]
        {
            new BrailleCircleButton(0.30f, 0.25f, 0.11f, 0.339f),  // Lane 0: 왼쪽 위   (W)
            new BrailleCircleButton(0.70f, 0.25f, 0.11f, 0.339f),  // Lane 1: 왼쪽 아래 (S)
            new BrailleCircleButton(0.30f, 0.75f, 0.11f, 0.339f),  // Lane 2: 오른쪽 위   (I)
            new BrailleCircleButton(0.70f, 0.75f, 0.11f, 0.339f),  // Lane 3: 오른쪽 아래 (K)
        };
    }

    // ── 기본 6버튼 배치 (왼쪽 1-3 / 오른쪽 4-6) — 원본 그대로, 인스펙터 비었을 때만 사용 ──

    static BrailleCircleButton[] DefaultButtons()
    {
        // 상단 4개 + 하단 2개
        return new BrailleCircleButton[]
        {
            new BrailleCircleButton(4.5f/16f,  5f/40f),   // Lane 1 (왼쪽 상단)
            new BrailleCircleButton(4.5f/16f, 15f/40f),   // Lane 2 (왼쪽 상단)
            new BrailleCircleButton(11f /16f, 10f/40f),   // Lane 3 (왼쪽 하단)
            new BrailleCircleButton(4.5f/16f, 25f/40f),   // Lane 4 (오른쪽 상단)
            new BrailleCircleButton(4.5f/16f, 35f/40f),   // Lane 5 (오른쪽 상단)
            new BrailleCircleButton(11f /16f, 30f/40f),   // Lane 6 (오른쪽 하단)
        };
    }
}