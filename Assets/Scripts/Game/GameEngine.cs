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

    [Header("버튼")]
    [Tooltip("버튼(레인) 개수. 값을 바꾸면 하단에 일자로 자동 재배치된다.")]
    [Min(1)] public int buttonCount = 4;
    [Tooltip("자동 배치된 버튼들. buttonCount와 길이가 다르면 일자로 다시 생성된다. " +
             "개별 위치/크기는 여기서 직접 수정 가능.")]
    public BrailleCircleButton[] idleButtons;

    [Header("상단 점자 텍스트 (계이름·박자 등)")]
    [Tooltip("디스플레이 위쪽에 점자로 띄울 텍스트")]
    public string topText = "do re mi";
    [Tooltip("점자 텍스트 점1의 행 위치")]
    public int topTextRow = 3;

    [Header("입력 & 사운드")]
    [Tooltip("키 입력 시 음을 낼 PianoPlayer")]
    public PianoPlayer piano;
    [Tooltip("레인별 입력 키 (배열 i번째 = 레인 i)")]
    public KeyCode[] laneKeys = { KeyCode.S, KeyCode.D, KeyCode.F, KeyCode.J };
    [Tooltip("레인별 계이름 (예: 도/레/미/파 또는 C4/D4...). 레인 수보다 적으면 순환 사용)")]
    public string[] laneNotes = { "도", "레", "미", "파" };

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

    void Awake() => EnsureButtons();

    /// <summary>idleButtons / laneFlash / display.buttons 를 항상 유효한 상태로 보장.
    /// buttonCount와 배열 길이가 다르면 일자로 자동 재배치한다.</summary>
    void EnsureButtons()
    {
        if (buttonCount < 1) buttonCount = 1;

        if (idleButtons == null || idleButtons.Length != buttonCount)
            idleButtons = LineButtons(buttonCount);

        if (laneFlash == null || laneFlash.Length != LaneCount)
            laneFlash = new float[LaneCount];

        if (display != null && display.buttons != idleButtons)
            display.buttons = idleButtons;
    }

    void Update()
    {
        if (display == null) return;
        EnsureButtons();

        if (!Application.isPlaying)
        {
            RenderIdleScreen();
            return;
        }

        HandleInput();

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

    // ── 입력 ──────────────────────────────────────────────────────────────────

    /// <summary>레인별 키 입력을 읽어 해당 레인을 누른다.</summary>
    void HandleInput()
    {
        if (laneKeys == null) return;
        int n = Mathf.Min(LaneCount, laneKeys.Length);
        for (int lane = 0; lane < n; lane++)
            if (Input.GetKeyDown(laneKeys[lane]))
                PressLane(lane);
    }

    /// <summary>레인을 누른다: 피아노 음 재생 + 버튼 점등 + (게임 중이면) 판정.</summary>
    public HitResult PressLane(int lane)
    {
        if (lane < 0 || lane >= LaneCount) return HitResult.None;

        // 1) 피아노 음
        if (piano != null && laneNotes != null && laneNotes.Length > 0)
            piano.PlayNote(laneNotes[lane % laneNotes.Length]);

        // 2) 버튼 점등 (아이들/게임 공통 시각 피드백)
        if (laneFlash != null && lane < laneFlash.Length)
            laneFlash[lane] = 0.15f;

        // 3) 게임 중이면 노트 판정
        return IsRunning ? TapLane(lane) : HitResult.None;
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
        RenderTopText();

        for (int lane = 0; lane < LaneCount; lane++)
        {
            idleButtons[lane].Draw(display);

            // 키를 눌러 점등된 버튼은 잠깐 하이라이트
            if (laneFlash != null && lane < laneFlash.Length && laneFlash[lane] > 0f)
            {
                laneFlash[lane] -= Time.deltaTime;
                idleButtons[lane].SetHighlight(display, true);
            }
        }
        display.Refresh();
    }

    /// <summary>디스플레이 상단 영역에 점자 텍스트를 렌더링한다.</summary>
    void RenderTopText()
    {
        if (string.IsNullOrEmpty(topText)) return;
        BrailleText.RenderCentered(display, topText, topTextRow);
    }

    void RenderFrame(float now)
    {
        display.ClearAll();
        RenderTopText();

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

    // ── 버튼 일자 자동 배치 ───────────────────────────────────────────────────

    /// <summary>
    /// 위쪽 절반은 점자 텍스트(계이름·박자)용으로 비워두고,
    /// 버튼 count개를 아래쪽에 가로로 균등하게 일렬 배치한다.
    /// 위치·크기는 모두 0~1 비율이라 그리드 해상도와 무관하게 같은 모양으로 배치된다.
    /// </summary>
    static BrailleCircleButton[] LineButtons(int count)
    {
        const float row    = 0.62f;    // 버튼 줄 (아래쪽 절반)
        const float radius = 0.13f;    // 행 높이 대비 반지름
        const float thick  = 0.24f;    // 반지름 대비 테두리 두께

        var buttons = new BrailleCircleButton[count];
        for (int i = 0; i < count; i++)
        {
            float colRatio = (i + 1f) / (count + 1f);   // 가로 균등 분배
            buttons[i] = new BrailleCircleButton(row, colRatio, radius, thick);
        }
        return buttons;
    }
}
