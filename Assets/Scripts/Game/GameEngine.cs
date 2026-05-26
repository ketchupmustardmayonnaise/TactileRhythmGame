using System.Collections.Generic;
using UnityEngine;

/// <summary>
/// 게임 핵심 로직. AudioManager와 BrailleCellDisplay를 연결해서
/// 노트 스폰 → 이동 → 판정 → 렌더링 루프를 처리한다.
/// 레인 수는 idleButtons 배열 길이로 결정된다.
/// </summary>
[ExecuteAlways]
public class GameEngine : MonoBehaviour
{
    [Header("References")]
    public BrailleCellDisplay display;
    public AudioManager audioManager;

    [Header("Game Settings")]
    [Tooltip("노트가 화면 맨 위에서 버튼까지 이동하는 시간(초)")]
    public float scrollSeconds = 2f;

    [Header("Timing Windows (초)")]
    public float windowPerfect = 0.07f;
    public float windowGood    = 0.14f;
    [Tooltip("버튼이 activation 0→1로 밝아지기 시작하는 노트 도착 전 시간(초)")]
    public float previewWindow = 0.5f;

    [Header("Idle Screen Buttons")]
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

    private int   TotalRows => display.Rows;
    private int   TotalCols => display.Columns;

    // ── Unity 생명주기 ────────────────────────────────────────────────────────

    void Awake()
    {
        if (idleButtons == null || idleButtons.Length == 0)
            idleButtons = new BrailleCircleButton[]
            {
                // rowRatio = 원래행/16, colRatio = 원래열/40
                new BrailleCircleButton(4.5f/16f,  5f/40f),
                new BrailleCircleButton(4.5f/16f, 15f/40f),
                new BrailleCircleButton(4.5f/16f, 25f/40f),
                new BrailleCircleButton(4.5f/16f, 35f/40f),
                new BrailleCircleButton(11f /16f, 10f/40f),
                new BrailleCircleButton(11f /16f, 30f/40f),
            };

        laneFlash = new float[LaneCount];

        if (display != null)
            display.buttons = idleButtons;
    }

    void Update()
    {
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
        UpdateNotePositions(now);
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
        laneFlash   = new float[LaneCount];
    }

    public void StartGame(float countdownSeconds = 3f)
    {
        if (song == null) return;
        IsRunning = true;

        AudioClip clip = Resources.Load<AudioClip>(song.audioFile);
        if (clip != null)
            audioManager.SchedulePlay(clip, countdownSeconds, song.offset);
        else
            Debug.LogWarning($"[GameEngine] 오디오 파일 없음: {song.audioFile}");
    }

    /// <summary>레인(= 버튼 인덱스)을 탭했을 때 호출. 판정 결과를 반환한다.</summary>
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

        if (best == null)               { laneFlash[lane] = 0.12f; return HitResult.None; }
        if (bestDiff <= windowPerfect)  { RegisterHit(best, lane, 300); return HitResult.Perfect; }
        if (bestDiff <= windowGood)     { RegisterHit(best, lane, 100); return HitResult.Good; }

        laneFlash[lane] = 0.12f;
        return HitResult.None;
    }

    // ── 내부 ─────────────────────────────────────────────────────────────────

    void RegisterHit(ActiveNote note, int lane, int baseScore)
    {
        note.isHit = true;
        Combo++;
        Score += baseScore + Combo * 5;
        laneFlash[lane] = 0.15f;
    }

    void SpawnNotes(float now)
    {
        float lookAhead = scrollSeconds + 0.1f;
        while (nextNoteIdx < song.notes.Count)
        {
            var nd = song.notes[nextNoteIdx];
            if (nd.time <= now + lookAhead) { activeNotes.Add(new ActiveNote(nd)); nextNoteIdx++; }
            else break;
        }
    }

    void UpdateNotePositions(float now)
    {
        foreach (var n in activeNotes)
        {
            int lane = n.data.lane;
            if (lane < 0 || lane >= LaneCount) continue;
            float hitRow   = idleButtons[lane].rowRatio * TotalRows;
            float timeLeft = n.data.time - now;
            // timeLeft=0 → hitRow, timeLeft=scrollSeconds → row 0
            n.rowPosition = hitRow - timeLeft * (hitRow / scrollSeconds);
        }
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

    // ── 렌더 ─────────────────────────────────────────────────────────────────

    void RenderIdleScreen()
    {
        if (display == null || idleButtons == null) return;
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

        // 2. 버튼 렌더 — 히트 플래시(파란색) 또는 activation 밝기
        for (int lane = 0; lane < LaneCount; lane++)
        {
            if (laneFlash[lane] > 0f)
            {
                laneFlash[lane] -= Time.deltaTime;
                idleButtons[lane].SetHighlight(display, true);
            }
            else
            {
                idleButtons[lane].DrawWithActivation(display, activations[lane]);
            }
        }

        // 3. 낙하 노트 (버튼 위까지만 그린다)
        int halfW = Mathf.Max(1, TotalCols / Mathf.Max(1, LaneCount) / 2 - 1);
        foreach (var n in activeNotes)
        {
            int lane = n.data.lane;
            if (lane < 0 || lane >= LaneCount || n.isHit) continue;

            int hitRow    = Mathf.RoundToInt(idleButtons[lane].rowRatio * TotalRows);
            int row       = Mathf.RoundToInt(n.rowPosition);
            if (row < 0 || row >= hitRow) continue;

            int centerCol = Mathf.RoundToInt(idleButtons[lane].colRatio * TotalCols);
            int cMin = Mathf.Max(0,             centerCol - halfW);
            int cMax = Mathf.Min(TotalCols - 1, centerCol + halfW);

            for (int dr = 0; dr <= 1; dr++)
            {
                int r2 = row + dr;
                if (r2 < 0 || r2 >= TotalRows) continue;
                for (int c = cMin; c <= cMax; c++)
                    display.SetDot(r2, c, true);
            }
        }

        display.Refresh();
    }
}
