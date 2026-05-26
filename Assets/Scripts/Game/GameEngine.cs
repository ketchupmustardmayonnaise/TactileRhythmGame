using System.Collections.Generic;
using UnityEngine;

/// <summary>
/// 게임 핵심 로직. AudioManager와 DotMatrixDisplay를 연결해서
/// 노트 스폰 → 이동 → 판정 → 렌더링 루프를 처리한다.
/// </summary>
public class GameEngine : MonoBehaviour
{
    [Header("References")]
    public BrailleCellDisplay display;
    public AudioManager audioManager;

    [Header("Game Settings")]
    [Tooltip("레인 수")]
    public int laneCount = 4;
    [Tooltip("노트가 화면 맨 위에서 히트존까지 이동하는 시간(초)")]
    public float scrollSeconds = 2f;

    [Header("Timing Windows (초)")]
    public float windowPerfect = 0.07f;
    public float windowGood    = 0.14f;

    // 공개 상태
    public int   Score  { get; private set; }
    public int   Combo  { get; private set; }
    public bool  IsRunning { get; private set; }

    private SongData song;
    private int nextNoteIdx;
    private readonly List<ActiveNote> activeNotes = new();

    // 렌더용 캐시
    private int HitRow    => display.Rows - 2;
    private int TotalRows => display.Rows;
    private int TotalCols => display.columns;
    private float RowsPerSecond => (HitRow) / scrollSeconds;

    // 레인별 히트 플래시 잔여 시간
    private float[] laneFlash;

    // --- 공개 API ---

    public void LoadSong(SongData songData)
    {
        song         = songData;
        nextNoteIdx  = 0;
        Score        = 0;
        Combo        = 0;
        IsRunning    = false;
        activeNotes.Clear();
        laneFlash = new float[laneCount];
    }

    /// <param name="countdownSeconds">카운트다운 후 오디오 재생까지 대기할 시간</param>
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

    /// <summary>레인을 탭했을 때 호출. 판정 결과를 반환한다.</summary>
    public HitResult TapLane(int lane)
    {
        if (!IsRunning || lane < 0 || lane >= laneCount) return HitResult.None;

        float now = (float)audioManager.SongTime;
        ActiveNote best = null;
        float bestDiff = float.MaxValue;

        foreach (var n in activeNotes)
        {
            if (n.data.lane != lane || n.isHit) continue;
            float diff = Mathf.Abs(n.data.time - now);
            if (diff < bestDiff) { bestDiff = diff; best = n; }
        }

        if (best == null)
        {
            laneFlash[lane] = 0.12f;
            return HitResult.None;
        }

        if (bestDiff <= windowPerfect)
        {
            RegisterHit(best, lane, 300);
            return HitResult.Perfect;
        }
        if (bestDiff <= windowGood)
        {
            RegisterHit(best, lane, 100);
            return HitResult.Good;
        }
        laneFlash[lane] = 0.12f;
        return HitResult.None;
    }

    // --- Unity 루프 ---

    void Update()
    {
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

    // --- 내부 ---

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
            if (nd.time <= now + lookAhead)
            {
                activeNotes.Add(new ActiveNote(nd));
                nextNoteIdx++;
            }
            else break;
        }
    }

    void UpdateNotePositions(float now)
    {
        foreach (var n in activeNotes)
        {
            float timeLeft = n.data.time - now;
            // timeLeft=0 이면 HitRow, timeLeft=scrollSeconds 이면 row 0
            n.rowPosition = HitRow - timeLeft * RowsPerSecond;
        }
    }

    void PruneMissedNotes(float now)
    {
        for (int i = activeNotes.Count - 1; i >= 0; i--)
        {
            var n = activeNotes[i];
            if (n.isHit || n.data.time < now - windowGood)
            {
                if (!n.isHit) Combo = 0;   // 미스
                activeNotes.RemoveAt(i);
            }
        }
    }

    // ──────────────────────────────────────────────────────────
    // 아이들 화면: 6개 원형 버튼 테두리만 표시
    // ──────────────────────────────────────────────────────────

    void RenderIdleScreen()
    {
        display.ClearAll();

        float ax = DotAspect();   // dotW / dotH (열 폭 ÷ 행 높이)

        const float CR = 1.3f;   // 원 반지름 (행 높이 단위)
        const float TH = 0.6f;   // 테두리 두께 (행 높이 단위)

        // 위쪽 4개 버튼 (row=4.5):
        //   왼쪽 2개 col=5, col=15 / 오른쪽 2개 col=25, col=35
        DrawCircleOutline(4.5f,  5f, CR, ax, TH);
        DrawCircleOutline(4.5f, 15f, CR, ax, TH);
        DrawCircleOutline(4.5f, 25f, CR, ax, TH);
        DrawCircleOutline(4.5f, 35f, CR, ax, TH);

        // 아래쪽 2개 버튼 (row=11):
        //   왼쪽 중앙 col=10 / 오른쪽 중앙 col=30
        DrawCircleOutline(11f, 10f, CR, ax, TH);
        DrawCircleOutline(11f, 30f, CR, ax, TH);

        display.Refresh();
    }

    /// <returns>dotW / dotH — 열 1칸이 행 1칸보다 화면에서 얼마나 좁은지 비율</returns>
    float DotAspect()
    {
        if (display == null) return 1f;
        var rect = display.GetComponent<RectTransform>().rect;
        if (rect.width <= 0 || rect.height <= 0) return 1f;
        return (rect.width / TotalCols) / (rect.height / TotalRows);
    }

    /// <summary>
    /// 시각적으로 원형이 되도록 aspect 보정하여 dot matrix에 원 테두리를 그린다.
    /// </summary>
    /// <param name="cr">중심 행</param>
    /// <param name="cc">중심 열</param>
    /// <param name="radius">반지름 (행 높이 기준)</param>
    /// <param name="aspectX">DotAspect() 값</param>
    /// <param name="thickness">테두리 두께 (행 높이 기준, 기본 0.6)</param>
    void DrawCircleOutline(float cr, float cc, float radius,
                           float aspectX, float thickness = 0.6f)
    {
        float colR = radius / aspectX;   // 열 단위 반지름

        int rMin = Mathf.Max(0,            Mathf.FloorToInt(cr - radius - 1));
        int rMax = Mathf.Min(TotalRows - 1, Mathf.CeilToInt(cr + radius + 1));
        int cMin = Mathf.Max(0,            Mathf.FloorToInt(cc - colR   - 1));
        int cMax = Mathf.Min(TotalCols - 1, Mathf.CeilToInt(cc + colR   + 1));

        for (int r = rMin; r <= rMax; r++)
            for (int c = cMin; c <= cMax; c++)
            {
                float dr   = r - cr;
                float dc   = (c - cc) * aspectX;   // 열 차이 → 행 높이 단위로 환산
                float dist = Mathf.Sqrt(dr * dr + dc * dc);
                if (Mathf.Abs(dist - radius) <= thickness)
                    display.SetDot(r, c, true);
            }
    }

    void RenderFrame(float now)
    {
        display.ClearAll();

        int colsPerLane = TotalCols / laneCount;

        // 레인 구분선
        for (int lane = 1; lane < laneCount; lane++)
        {
            int sepCol = lane * colsPerLane;
            for (int r = 0; r < TotalRows; r++)
                display.SetDot(r, sepCol, true);
        }

        // 히트존 라인
        for (int c = 0; c < TotalCols; c++)
            display.SetDot(HitRow, c, true);

        // 히트 플래시 (파란색 하이라이트)
        for (int lane = 0; lane < laneCount; lane++)
        {
            if (laneFlash[lane] > 0f)
            {
                laneFlash[lane] -= Time.deltaTime;
                int start = lane * colsPerLane + 1;
                int end   = start + colsPerLane - 2;
                for (int c = start; c <= end; c++)
                {
                    display.SetDot(HitRow + 1, c, true);
                    display.SetDotHighlight(HitRow + 1, c, true);
                }
            }
        }

        // 노트
        foreach (var n in activeNotes)
        {
            int row = Mathf.RoundToInt(n.rowPosition);
            if (row < 0 || row > HitRow) continue;

            int laneStart = n.data.lane * colsPerLane + 1;
            int laneEnd   = laneStart + colsPerLane - 2;

            // 노트 모양: 두 줄 높이 블록
            for (int dr = 0; dr <= 1; dr++)
            {
                int r2 = row + dr;
                if (r2 < 0 || r2 >= TotalRows) continue;
                for (int c = laneStart; c <= laneEnd; c++)
                    display.SetDot(r2, c, true);
            }
        }

        display.Refresh();
    }
}
