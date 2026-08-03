using System.Collections;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.SceneManagement;
using TMPro;

/// <summary>
/// 게임 씬을 총괄한다. (2key 전용)
/// - 시작 메뉴: [Enter/Space] 게임 시작(2-Key: D/K) · [C] 타이밍 조정
/// - 채보 파일명 규칙: {songResourceName}_2k  (없으면 무접미사 파일로 폴백)
/// - 곡 전환: 인스펙터의 Song Resource Name 값을 바꾼다.
/// - 판정: Perfect / Good / 그 외 전부 Miss. 판정마다 engine.OnJudge 로 집계.
/// - 곡 종료: engine.OnFinished → 결과창(ResultScreen) 표시.
/// - ESC: 씬을 리로드해 메뉴로 복귀.
/// - '타이밍 조정': 예고(채워짐)에 맞춰 노트를 치면 반응 지연을 자동 측정해
///                  engine.previewOffset(예고 오프셋)을 PlayerPrefs에 저장한다.
/// - 점수: 채보 노트 수와 무관하게 MAX_SCORE(만점)로 정규화해 표시.
/// </summary>
public class GameScreen : MonoBehaviour
{
    [Header("Core")]
    public GameEngine engine;

    [Header("Result (곡 종료 결과창)")]
    public ResultScreen resultScreen;

    [Header("HUD (선택)")]
    public TextMeshProUGUI scoreText;
    public TextMeshProUGUI comboText;
    public TextMeshProUGUI judgmentText;

    [Header("Menu (선택)")]
    [Tooltip("메뉴 전체 오브젝트(검정 배경 Image + 텍스트의 상위 = 'Menu'). " +
             "게임이 시작되면 꺼지고, 메뉴로 돌아오면 다시 켜진다. " +
             "비워두면 menuText 오브젝트를 대신 토글한다.")]
    public GameObject menuRoot;
    [Tooltip("메뉴/타이밍 조정 안내 텍스트. 없어도 Enter/C 키로 동작한다.")]
    public TextMeshProUGUI menuText;

    // menuRoot 에 붙은 검정 배경 Image(있으면). 타이밍 조정 중에는 이 배경만 잠깐 꺼서
    // 뒤의 노트(점자 채워짐)가 보이도록 한다. Start에서 1회 캐싱.
    private UnityEngine.UI.Image menuBg;

    [Tooltip("곡 전환은 여기를 바꾼다. Resources/Songs/ 안 JSON 파일명 접두(확장자·접미사 제외). 예: heavy_serenade")]
    public string songResourceName = "heavy_serenade";

    [Tooltip("카운트다운(초). 0이면 즉시 시작")]
    public float countdownSeconds = 3f;

    // ── 점수 정규화 설정 ──────────────────────────────────────────────────────
    private const int   MAX_SCORE      = 1_000_000; // 정규화 목표 만점
    private const float PERFECT_WEIGHT = 1.0f;      // Perfect 1개당 기여 비율
    private const float GOOD_WEIGHT    = 0.5f;      // Good 1개당 기여 비율

    private int   totalNotes;      // 현재 채보의 총 노트 수
    private float achievedWeight;  // 누적 획득 가중치
    private int   normalizedScore; // 표시용 정규화 점수(0 ~ MAX_SCORE)

    // ── 판정 개수 집계 ────────────────────────────────────────────────────────
    private int perfectCount, goodCount, missCount;

    // ── 2key 키 배치 (D / K) ──────────────────────────────────────────────────
    private static readonly KeyCode[] Keys2 = { KeyCode.D, KeyCode.K };
    private KeyCode[] activeKeys = Keys2;

    private bool  inMenu = true;
    private float judgmentTimer;

    // ── 타이밍 조정 상태 ──────────────────────────────────────────────────────
    private bool calibrating;
    private bool calibFinalizing;
    private readonly List<float> calibSamples = new();
    private const int   CALIB_INTERVAL_BEATS = 24;    // 채보에 넣을 노트 수
    private const float CALIB_INTERVAL       = 0.5f;  // 노트 간격(초) = 120BPM
    private const float CALIB_FIRST          = 1.0f;  // 첫 노트 시각(초)
    private const float CALIB_PREVIEW        = 0.5f;  // 예고 시간(초)
    private const int   CALIB_TARGET         = 16;    // 목표 표본 수(모이면 종료)
    private const int   CALIB_MIN            = 4;     // 유효 최소 표본 수

    // ──────────────────────────────────────────────────────────────────────────

    void Start()
    {
        if (engine != null)
        {
            engine.OnJudge    += HandleJudge;
            engine.OnFinished += HandleFinished;
        }
        if (menuRoot != null) menuBg = menuRoot.GetComponent<UnityEngine.UI.Image>();
        ShowMenu();
    }

    // ── 메뉴 표시/숨김 헬퍼 ───────────────────────────────────────────────────
    // menuRoot(검정 배경 + 텍스트 상위)를 통째로 켜고 끈다.
    // menuRoot가 지정 안 됐으면 예전처럼 menuText 오브젝트만 토글한다.

    /// <summary>메뉴 전체(배경 포함)를 켜고 끈다.</summary>
    void SetMenuActive(bool on)
    {
        if (menuRoot != null) menuRoot.SetActive(on);
        else if (menuText != null) menuText.gameObject.SetActive(on);
    }

    /// <summary>검정 배경만 켜고 끈다(타이밍 조정 중엔 꺼서 노트가 보이게).</summary>
    void SetMenuBackground(bool on)
    {
        if (menuBg != null) menuBg.enabled = on;
    }

    void OnDestroy()
    {
        if (engine != null)
        {
            engine.OnJudge    -= HandleJudge;
            engine.OnFinished -= HandleFinished;
        }
    }

    // ── 채보 존재 확인 ────────────────────────────────────────────────────────

    /// <summary>2key 채보 리소스명을 조용히 찾는다(없으면 null).</summary>
    static string ResolveChart(string baseName)
    {
        string primary = baseName + "_2k";
        if (Resources.Load<TextAsset>($"Songs/{primary}") != null) return primary;
        if (Resources.Load<TextAsset>($"Songs/{baseName}") != null) return baseName; // 폴백
        return null;
    }

    // ── 메뉴 ──────────────────────────────────────────────────────────────────

    void ShowMenu(string notice = null)
    {
        inMenu = true;
        calibrating = false;

        SetMenuActive(true);        // 메뉴 전체(검정 배경 포함) 켜기
        SetMenuBackground(true);    // 배경도 다시 켜기(타이밍 조정에서 꺼졌을 수 있음)

        if (!menuText)
        {
            if (!string.IsNullOrEmpty(notice)) Debug.LogWarning($"[GameScreen] {notice}");
            return;
        }

        string avail  = ResolveChart(songResourceName) != null ? "" : "   (채보 없음)";
        string offStr = OffsetLabel(PlayerPrefs.GetFloat(GameEngine.PrefKeyPreviewOffset, 0f));
        string head   = string.IsNullOrEmpty(notice) ? "" : $"<color=#ff6666>{notice}</color>\n\n";

        menuText.gameObject.SetActive(true);
        menuText.text =
            head +
            "RHYTHM GAME (2-Key)\n\n" +
            $"곡: {songResourceName}{avail}\n\n" +
            "[Enter] 게임 시작    (D / K)\n" +
            "[C] 타이밍 조정\n\n" +
            $"현재 예고 보정: {offStr}";
    }

    static string OffsetLabel(float sec)
    {
        if (Mathf.Abs(sec) < 0.0005f) return "없음";
        return (sec >= 0 ? "+" : "") + $"{sec * 1000f:0}ms";
    }

    // ── 일반 플레이 시작 ──────────────────────────────────────────────────────

    public void SelectPlay()
    {
        string chart = ResolveChart(songResourceName);
        if (chart == null) { ShowMenu($"'{songResourceName}'의 2-Key 채보를 찾을 수 없습니다."); return; }

        SongData song = SongLoader.LoadFromResources(chart);
        if (song == null || string.IsNullOrEmpty(song.source))
        { ShowMenu($"'{chart}.json' 로드 실패 — source 필드를 확인하세요."); return; }

        ResetCounters(song);
        engine.LoadSong(song);
        engine.StartGame(countdownSeconds);

        inMenu = false;
        SetMenuActive(false);   // 게임 시작 → 메뉴(검정 배경 + 텍스트) 통째로 숨김
    }

    void ResetCounters(SongData song)
    {
        totalNotes    = (song?.notes != null) ? song.notes.Count : 0;
        achievedWeight = 0f;
        normalizedScore = 0;
        perfectCount = goodCount = missCount = 0;
    }

    // ── 루프 ──────────────────────────────────────────────────────────────────

    void Update()
    {
        // 결과창 표시 중이면 입력은 ResultScreen이 처리 → 여기선 아무 것도 안 함
        if (resultScreen != null && resultScreen.IsShowing) return;

        if (inMenu)
        {
            if (Input.GetKeyDown(KeyCode.Return) || Input.GetKeyDown(KeyCode.KeypadEnter)
                || Input.GetKeyDown(KeyCode.Space))
                SelectPlay();
            else if (Input.GetKeyDown(KeyCode.C))
                StartCalibration();
            return;
        }

        // ESC → 메뉴로 (씬 리로드)
        if (Input.GetKeyDown(KeyCode.Escape)) { ReloadScene(); return; }

        if (calibrating) { UpdateCalibration(); return; }

        // ── 일반 플레이 ──
        if (scoreText) scoreText.text = $"SCORE\n{normalizedScore:D7}";
        if (comboText) comboText.text = engine.Combo > 1 ? $"{engine.Combo} COMBO" : "";
        if (judgmentText && judgmentTimer > 0f)
        {
            judgmentTimer -= Time.deltaTime;
            if (judgmentTimer <= 0f) judgmentText.text = "";
        }

        for (int i = 0; i < activeKeys.Length && i < engine.LaneCount; i++)
            if (Input.GetKeyDown(activeKeys[i])) HandleTap(i);
    }

    void HandleTap(int lane)
    {
        if (SfxPlayer.Instance != null) SfxPlayer.Instance.PlayClap();
        engine.TapLane(lane);   // 채점/개수는 engine.OnJudge → HandleJudge 에서 처리
    }

    // ── 판정 이벤트 (Perfect / Good / Miss) ───────────────────────────────────

    void HandleJudge(HitResult result)
    {
        if (calibrating) return;   // 타이밍 조정 중엔 집계 안 함

        switch (result)
        {
            case HitResult.Perfect:
                perfectCount++; achievedWeight += PERFECT_WEIGHT;
                ShowJudgment("PERFECT", new Color(0.4f, 1f, 1f)); break;
            case HitResult.Good:
                goodCount++;    achievedWeight += GOOD_WEIGHT;
                ShowJudgment("GOOD", new Color(1f, 0.9f, 0.3f)); break;
            case HitResult.Miss:
                missCount++;
                ShowJudgment("MISS", new Color(1f, 0.4f, 0.4f)); break;
        }
        RecalcScore();
    }

    void HandleFinished()
    {
        if (calibrating) { FinalizeCalibration(); return; }

        // 결과창 표시
        if (resultScreen != null)
            resultScreen.Show(normalizedScore, perfectCount, goodCount, missCount, engine.MaxCombo);
        else
            Debug.Log($"[RESULT] score={normalizedScore} P={perfectCount} G={goodCount} " +
                      $"M={missCount} maxCombo={engine.MaxCombo}");
    }

    void RecalcScore()
    {
        float maxWeight = totalNotes * PERFECT_WEIGHT;
        normalizedScore = (maxWeight > 0f)
            ? Mathf.RoundToInt(MAX_SCORE * (achievedWeight / maxWeight))
            : 0;
    }

    void ShowJudgment(string text, Color color)
    {
        if (!judgmentText) return;
        judgmentText.text  = text;
        judgmentText.color = color;
        judgmentTimer = 0.5f;
    }

    // ── 타이밍 조정 ────────────────────────────────────────────────────────────

    void StartCalibration()
    {
        calibrating = true;
        calibFinalizing = false;
        inMenu = false;
        calibSamples.Clear();

        // 측정은 예고 오프셋 0(예고가 노트 시각에 정확히 완료) 상태에서.
        engine.previewOffset = 0f;

        SongData chart = BuildCalibrationChart();
        engine.LoadSong(chart);
        engine.previewWindow = CALIB_PREVIEW;   // 예고 시간 고정

        AudioClip silent = BuildSilentClip(CALIB_FIRST + CALIB_INTERVAL_BEATS * CALIB_INTERVAL + 2f);
        engine.StartGameWithClip(silent, countdownSeconds);

        // 타이밍 조정 중엔 뒤의 노트(채워짐)를 봐야 하므로 검정 배경은 끄고 텍스트만 남긴다.
        SetMenuActive(true);
        SetMenuBackground(false);
        if (menuText)
        {
            menuText.gameObject.SetActive(true);
            RenderCalibText();
        }
    }

    void UpdateCalibration()
    {
        if (calibFinalizing) return;   // 종료 메시지 표시 중 → 입력 무시

        // 예고(채워짐)에 맞춰 노트를 치면 부호 있는 오차를 표본으로 수집
        for (int i = 0; i < activeKeys.Length && i < engine.LaneCount; i++)
        {
            if (!Input.GetKeyDown(activeKeys[i])) continue;
            if (SfxPlayer.Instance != null) SfxPlayer.Instance.PlayClap();

            if (engine.TryMeasureTap(i, out float err))
            {
                calibSamples.Add(err);
                RenderCalibText();
                if (calibSamples.Count >= CALIB_TARGET) { FinalizeCalibration(); return; }
            }
        }
    }

    void RenderCalibText()
    {
        if (!menuText) return;
        menuText.gameObject.SetActive(true);
        menuText.text =
            "타이밍 조정\n\n" +
            "버튼 안이 가득 찰 때 맞춰\n" +
            "D / K 를 리듬에 맞게 두드리세요.\n\n" +
            $"입력: {calibSamples.Count} / {CALIB_TARGET}\n\n" +
            "ESC : 취소";
    }

    void FinalizeCalibration()
    {
        if (!calibrating || calibFinalizing) return;
        calibFinalizing = true;      // calibrating 은 리로드까지 true로 유지(집계 차단)
        engine.Halt();               // 노트 진행 정지(잔여 miss 깜빡임 방지)

        if (calibSamples.Count < CALIB_MIN)
        {
            StartCoroutine(CalibDoneThenMenu("표본이 부족합니다. 다시 시도하세요.", false));
            return;
        }

        float offset = Mathf.Clamp(Median(calibSamples), -0.2f, 0.2f);
        PlayerPrefs.SetFloat(GameEngine.PrefKeyPreviewOffset, offset);
        PlayerPrefs.Save();

        StartCoroutine(CalibDoneThenMenu(
            $"보정 완료: {OffsetLabel(offset)}\n(예고를 {(offset >= 0 ? "앞당김" : "늦춤")})",
            true));
    }

    IEnumerator CalibDoneThenMenu(string msg, bool success)
    {
        SetMenuActive(true);
        SetMenuBackground(true);   // 완료 메시지를 잘 보이게 배경 복원
        if (menuText)
        {
            menuText.gameObject.SetActive(true);
            menuText.text = (success ? "" : "<color=#ff6666>") + msg + (success ? "" : "</color>");
        }
        if (AudioManager.Instance != null) AudioManager.Instance.Stop();
        yield return new WaitForSecondsRealtime(1.4f);
        ReloadScene();   // 저장된 오프셋은 리로드 후 GameEngine.Awake에서 다시 로드됨
    }

    static float Median(List<float> values)
    {
        var v = new List<float>(values);
        v.Sort();
        int n = v.Count;
        return (n % 2 == 1) ? v[n / 2] : 0.5f * (v[n / 2 - 1] + v[n / 2]);
    }

    /// <summary>일정 간격의 2key 조정용 채보를 코드로 생성.</summary>
    static SongData BuildCalibrationChart()
    {
        var song = new SongData
        {
            source     = "",                 // 오디오는 무음 클립으로 대체
            difficulty = "calibration",
            meta       = new SongMeta { bpm = 120f, seconds_per_beat = CALIB_INTERVAL },
            notes      = new List<NoteData>(),
        };
        for (int i = 0; i < CALIB_INTERVAL_BEATS; i++)
        {
            song.notes.Add(new NoteData
            {
                time = CALIB_FIRST + i * CALIB_INTERVAL,
                lane = i % 2,               // 0(D) / 1(K) 번갈아 — 이미 0-based
            });
        }
        return song;
    }

    /// <summary>지정 길이(초)의 무음 AudioClip 생성. SongTime 클럭 용도.</summary>
    static AudioClip BuildSilentClip(float lengthSeconds)
    {
        const int rate = 44100;
        int samples = Mathf.Max(1, Mathf.CeilToInt(lengthSeconds * rate));
        return AudioClip.Create("calib_silent", samples, 1, rate, false);
    }

    // ── 공통 ──────────────────────────────────────────────────────────────────

    void ReloadScene()
    {
        if (AudioManager.Instance != null) AudioManager.Instance.Stop();
        SceneManager.LoadScene(SceneManager.GetActiveScene().buildIndex);
    }
}
