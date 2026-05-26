using System.Collections.Generic;
using UnityEngine;

/// <summary>
/// 게임 핵심 로직. AudioManager와 BrailleCellDisplay를 연결해서
/// 노트 스폰 → 이동 → 판정 → 렌더링 루프를 처리한다.
/// 레인 수 = idleButtons 배열 길이 (기본 6: 왼쪽 1-3, 오른쪽 4-6).
/// </summary>
[ExecuteAlways]
public class GameEngine : MonoBehaviour
{
    [Header("References")]
    public BrailleCellDisplay display;
    public AudioManager audioManager;

    [Header("Timing Windows (초)")]
    public float windowPerfect = 0.07f;
    public float windowGood    = 0.14f;
    [Tooltip("버튼 activation 0→1 시간(초). LoadSong 시 seconds_per_beat로 자동 설정됨")]
    public float previewWindow = 0.5f;

    [Header("Idle Screen Buttons  (왼쪽 1-3, 오른쪽 4-6)")]
    public BrailleCircleButton[] idleButtons;

    // 공개 상태
    public int  Score     { get; private set; }
    public int  Combo     { get; private set; }
    public bool IsRunning { get; private set; }

    /// <summary>레인 수 = 버튼 수</summary>
    public int LaneCount => idleButtons != null ? idleButtons.Length : 0;

    private SongData song;
    private int nextNoteIdx;
    private readonly List<ActiveNote> activeNotes = new();
    private float[] laneFlash;

    // ── Unity 생명주기 ────────────────────────────────────────────────────────

    void Awake()
    {
        if (idleButtons == null || idleButtons.Length == 0)
            idleButtons = DefaultButtons();

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

    public void LoadSong(SongData songData)
    {
        song        = songData;
        nextNoteIdx = 0;
        Score       = 0;
        Combo       = 0;
        IsRunning   = false;
        activeNotes.Clear();
        laneFlash = new float[LaneCount];

        // seconds_per_beat → previewWindow
        if (song.meta != null && song.meta.seconds_per_beat > 0f)
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
        ActiveNote best     = null;
        float      bestDiff = float.MaxValue;

        foreach (var n in activeNotes)
        {
            if (n.data.lane != lane || n.isHit) continue;
            float diff = Mathf.Abs(n.data.time - now);
            if (diff < bestDiff) { bestDiff = diff; best = n; }
        }

        if (best == null)              { laneFlash[lane] = 0.12f; return HitResult.None; }
        if (bestDiff <= windowPerfect) { RegisterHit(best, lane, 300); return HitResult.Perfect; }
        if (bestDiff <= windowGood)    { RegisterHit(best, lane, 100); return HitResult.Good; }

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

    // ── 기본 6버튼 배치 (왼쪽 1-3 / 오른쪽 4-6) ────────────────────────────

    static BrailleCircleButton[] DefaultButtons()
    {
        // 기존 배치 그대로 — 상단 4개 + 하단 2개
        // 왼쪽(Lane 1-3): col 5, 15, 10  / 오른쪽(Lane 4-6): col 25, 35, 30
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
