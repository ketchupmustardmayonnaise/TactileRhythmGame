using System.Collections;
using System.Collections.Generic;
using UnityEngine;
using TMPro;

/// <summary>
/// 게임 씬을 총괄한다. (Classic / Easy / Single)
/// - 시작 메뉴: [Enter/Space] 게임 시작 · [C] 타이밍 조정 · [F2] 오토 켜기/끄기
/// - 채보 파일명 규칙: {songResourceName} + RhythmTestModes.ChartSuffix
/// - 곡 전환: 인스펙터의 Song Resource Name 값을 바꾼다.
/// - 판정: Perfect / Good / 그 외 전부 Miss. 판정마다 engine.OnJudge 로 집계.
/// - 곡 종료: engine.OnFinished → 결과창(ResultScreen) 표시.
/// - ESC: 선택한 테스트 모드를 유지하고 메뉴로 복귀.
/// - '타이밍 조정': 예고(채워짐)에 맞춰 노트를 치면 반응 지연을 자동 측정해
///                  engine.previewOffset(예고 오프셋)을 PlayerPrefs에 저장한다.
/// - 점수: 채보 노트 수와 무관하게 MAX_SCORE(만점)로 정규화해 표시.
/// </summary>
public class GameScreen : MonoBehaviour
{
    [Header("Core")]
    public GameEngine engine;

    [Header("Presentation / Debug")]
    [Tooltip("시각 버전. 꺼짐(기본): 텍스트는 Console로 출력하고 파란 타격 효과를 숨깁니다. 켜짐: 텍스트 UI와 파란 효과를 표시합니다. 노트 예고와 Auto 표시는 항상 유지합니다.")]
    public bool visualVersion = false;
    private readonly GameTextOutput textOutput = new();
    private bool appliedVisualVersion;
    private bool menuBackgroundVisible = true;
    private string menuMessage = "";
    private AutoPlayIndicator autoIndicator;

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
    private const int MAX_SCORE = 1_000_000; // 정규화 목표 만점
    private const float PERFECT_WEIGHT = 1.0f;      // Perfect 1개당 기여 비율
    private const float GOOD_WEIGHT = 0.5f;      // Good 1개당 기여 비율

    private int totalNotes;      // 현재 채보의 총 노트 수
    private float achievedWeight;  // 누적 획득 가중치
    private int normalizedScore; // 표시용 정규화 점수(0 ~ MAX_SCORE)

    // ── 판정 개수 집계 ────────────────────────────────────────────────────────
    private int perfectCount, goodCount, missCount;

    private RhythmTestMode activeMode;
    private string InputHint => RhythmTestModes.InputHint(activeMode);
    private bool IsLaneKeyDown(int lane) => RhythmTestModes.IsLaneKeyDown(activeMode, lane);

    private bool inMenu = true;
    private int menuInputAfterFrame = -1;
    private float judgmentTimer;

    // ── 타이밍 조정 상태 ──────────────────────────────────────────────────────
    private bool calibrating;
    private bool calibFinalizing;
    private readonly List<float> calibSamples = new();
    private const int CALIB_INTERVAL_BEATS = 24;    // 채보에 넣을 노트 수
    private const float CALIB_FIRST = 1.0f;  // 첫 노트 시각(초)
    private float CalibrationInterval => RhythmTestModes.CalibrationInterval(activeMode);
    private float previewBeforeCalibration;
    private AudioClip calibrationClip;
    private const int CALIB_TARGET = 16;    // 목표 표본 수(모이면 종료)
    private const int CALIB_MIN = 4;     // 유효 최소 표본 수

    // ──────────────────────────────────────────────────────────────────────────

    void Start()
    {
        if (engine != null)
        {
            activeMode = engine.testMode;
            engine.OnJudge += HandleJudge;
            engine.OnFinished += HandleFinished;
        }
        if (resultScreen != null) resultScreen.MenuRequested += ReturnToMenu;
        if (menuRoot != null) menuBg = menuRoot.GetComponent<UnityEngine.UI.Image>();
        var canvas = engine != null && engine.display != null
            ? engine.display.GetComponentInParent<Canvas>()
            : menuText != null ? menuText.GetComponentInParent<Canvas>() : null;
        if (canvas != null) autoIndicator = AutoPlayIndicator.Create(canvas.rootCanvas);
        ApplyPresentationMode();
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
        menuBackgroundVisible = on;
        if (menuBg != null) menuBg.enabled = on && visualVersion;
    }

    // 표시와 게임 동작의 모드 설정은 여기서 함께 적용한다.
    // 향후 디버깅 전용 동작도 이 경로에 연결할 수 있다.
    void ApplyPresentationMode()
    {
        var overlays = new List<UnityEngine.UI.Graphic>();
        if (menuBg != null) overlays.Add(menuBg);
        if (resultScreen != null)
        {
            var root = resultScreen.panelRoot != null ? resultScreen.panelRoot : resultScreen.gameObject;
            overlays.AddRange(root.GetComponentsInChildren<UnityEngine.UI.Graphic>(true));
            resultScreen.TextOutput = textOutput;
        }
        textOutput.SetMode(!visualVersion, this, overlays);
        if (engine != null) engine.SetVisualVersion(visualVersion);
        appliedVisualVersion = visualVersion;
        SetMenuBackground(menuBackgroundVisible);
        if (inMenu || calibrating) textOutput.Write("Menu", menuText, menuMessage);
        if (scoreText) textOutput.Write("Score", scoreText, scoreText.text);
        if (comboText) textOutput.Write("Combo", comboText, comboText.text);
        if (judgmentText) textOutput.Write("Judge", judgmentText, judgmentText.text);
        if (resultScreen != null) resultScreen.RefreshPresentation();
        var managed = new HashSet<TMP_Text> { menuText, scoreText, comboText, judgmentText };
        if (resultScreen != null) { managed.Add(resultScreen.titleText); managed.Add(resultScreen.bodyText); }
        textOutput.PublishOtherText(this, managed);
    }

    void WriteMenu(string message)
    {
        menuMessage = message;
        textOutput.Write("Menu", menuText, message);
    }

    void OnDestroy()
    {
        if (autoIndicator != null) Destroy(autoIndicator.gameObject);
        textOutput.RestoreVisibility();
        if (resultScreen != null) resultScreen.MenuRequested -= ReturnToMenu;
        if (calibrationClip != null) Destroy(calibrationClip);
        if (engine != null)
        {
            engine.OnJudge -= HandleJudge;
            engine.OnFinished -= HandleFinished;
        }
    }

    // ── 채보 존재 확인 ────────────────────────────────────────────────────────

    /// <summary>선택한 모드의 채보만 로드한다. Easy/Single은 Classic으로 대체하지 않는다.</summary>
    string ResolveChart(string baseName)
    {
        string primary = baseName + RhythmTestModes.ChartSuffix(activeMode);
        if (Resources.Load<TextAsset>($"Songs/{primary}") != null) return primary;
        if (RhythmTestModes.AllowLegacyChartFallback(activeMode)
            && Resources.Load<TextAsset>($"Songs/{baseName}") != null) return baseName;
        return null;
    }

    // ── 메뉴 ──────────────────────────────────────────────────────────────────

    void ShowMenu(string notice = null)
    {
        inMenu = true;
        calibrating = false;

        SetMenuActive(true);        // 메뉴 전체(검정 배경 포함) 켜기
        SetMenuBackground(true);    // 배경도 다시 켜기(타이밍 조정에서 꺼졌을 수 있음)

        string avail = ResolveChart(songResourceName) != null ? "" : "   (채보 없음)";
        string offStr = OffsetLabel(PlayerPrefs.GetFloat(engine.OffsetPreferenceKey, 0f));
        string head = string.IsNullOrEmpty(notice) ? "" : $"<color=#ff6666>{notice}</color>\n\n";

        if (menuText) menuText.gameObject.SetActive(true);
        WriteMenu(
            head +
            $"RHYTHM GAME ({engine.LaneCount}-Key) · {activeMode}\n\n" +
            $"곡: {songResourceName}{avail}\n\n" +
            $"[Enter] 게임 시작    ({InputHint})\n" +
            "[C] 타이밍 조정\n" +
            $"[F2] AUTO: {(engine.AutoPlayEnabled ? "ON" : "OFF")}\n\n" +
            $"현재 예고 보정: {offStr}");
    }

    static string OffsetLabel(float sec)
    {
        if (Mathf.Abs(sec) < 0.0005f) return "없음";
        return (sec >= 0 ? "+" : "") + $"{sec * 1000f:0}ms";
    }

    // ── 일반 플레이 시작 ──────────────────────────────────────────────────────

    /// <summary>시작 메뉴의 F2 또는 UI Button에서 호출할 수 있다.</summary>
    public void ToggleAutoPlay()
    {
        if (engine == null || !inMenu || (resultScreen != null && resultScreen.IsShowing)) return;
        engine.SetAutoPlay(!engine.AutoPlayEnabled);
        ShowMenu();
    }

    void LateUpdate()
    {
        if (autoIndicator != null)
            autoIndicator.gameObject.SetActive(engine != null && engine.IsAutoPlayActive);
    }

    public void SelectPlay()
    {
        string chart = ResolveChart(songResourceName);
        if (chart == null) { ShowMenu($"'{songResourceName}'의 {activeMode} 채보를 찾을 수 없습니다."); return; }

        SongData song = SongLoader.LoadFromResources(chart);
        if (song == null || string.IsNullOrEmpty(song.source))
        { ShowMenu($"'{chart}.json' 로드 실패 — source 필드를 확인하세요."); return; }

        engine.LoadSong(song);
        ResetCounters(song);
        engine.StartGame(countdownSeconds);
        if (!engine.IsRunning) { ShowMenu("오디오를 재생할 수 없습니다."); return; }

        inMenu = false;
        SetMenuActive(false);   // 게임 시작 → 메뉴(검정 배경 + 텍스트) 통째로 숨김
    }

    void ResetCounters(SongData song)
    {
        totalNotes = (song?.notes != null) ? song.notes.Count : 0;
        achievedWeight = 0f;
        normalizedScore = 0;
        perfectCount = goodCount = missCount = 0;
    }

    // ── 루프 ──────────────────────────────────────────────────────────────────

    void Update()
    {
        if (engine == null) return;
        if (appliedVisualVersion != visualVersion) ApplyPresentationMode();
        if (activeMode != engine.testMode)
        {
            ReturnToMenu();
            return;
        }
        // 결과창 표시 중이면 입력은 ResultScreen이 처리 → 여기선 아무 것도 안 함
        if (resultScreen != null && resultScreen.IsShowing) return;

        if (inMenu)
        {
            // 결과창을 닫은 Space/Enter가 같은 프레임에 게임까지 시작하지 않도록 한다.
            if (Time.frameCount <= menuInputAfterFrame) return;
            if (Input.GetKeyDown(KeyCode.Return) || Input.GetKeyDown(KeyCode.KeypadEnter)
                || Input.GetKeyDown(KeyCode.Space))
                SelectPlay();
            else if (Input.GetKeyDown(KeyCode.C))
                StartCalibration();
            else if (Input.GetKeyDown(KeyCode.F2))
                ToggleAutoPlay();
            return;
        }

        if (Input.GetKeyDown(KeyCode.Escape)) { ReturnToMenu(); return; }

        if (calibrating) { UpdateCalibration(); return; }

        // ── 일반 플레이 ──
        textOutput.Write("Score", scoreText, $"SCORE\n{normalizedScore:D7}");
        textOutput.Write("Combo", comboText, engine.Combo > 1 ? $"{engine.Combo} COMBO" : "");
        if (judgmentTimer > 0f)
        {
            judgmentTimer -= Time.deltaTime;
            if (judgmentTimer <= 0f) textOutput.Write("Judge", judgmentText, "");
        }

        // 오토는 노트 입력만 무시한다. 위의 메뉴/취소 입력 처리는 그대로 둔다.
        if (!engine.IsAutoPlayActive)
            for (int i = 0; i < engine.LaneCount; i++)
                if (IsLaneKeyDown(i)) HandleTap(i);
    }

    void HandleTap(int lane)
    {
        // 소리는 여기(입력 시점)가 아니라 판정 확정(HandleJudge)에서 결과별로 재생한다.
        engine.TapLane(lane);   // 채점/개수/소리는 engine.OnJudge → HandleJudge 에서 처리
    }

    // ── 판정 이벤트 (Perfect / Good / Miss) ───────────────────────────────────

    void HandleJudge(HitResult result)
    {
        if (calibrating) return;   // 타이밍 조정 중엔 집계 안 함

        var sfx = SfxPlayer.Instance;
        switch (result)
        {
            case HitResult.Perfect:
                perfectCount++; achievedWeight += PERFECT_WEIGHT;
                if (sfx != null) sfx.PlayPerfect();
                ShowJudgment("PERFECT", new Color(0.4f, 1f, 1f)); break;
            case HitResult.Good:
                goodCount++; achievedWeight += GOOD_WEIGHT;
                if (sfx != null) sfx.PlayGood();     // 기존 박수 소리
                ShowJudgment("GOOD", new Color(1f, 0.9f, 0.3f)); break;
            case HitResult.Miss:
                missCount++;
                if (sfx != null) sfx.PlayMiss();     // 빗맞힘 + 놓쳐 지나감 둘 다 여기서
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
        textOutput.Write("Judge", judgmentText, text, true);
        if (judgmentText) judgmentText.color = color;
        judgmentTimer = 0.5f;
    }

    // ── 타이밍 조정 ────────────────────────────────────────────────────────────

    void StartCalibration()
    {
        previewBeforeCalibration = engine.previewWindow;
        calibrating = true;
        calibFinalizing = false;
        inMenu = false;
        calibSamples.Clear();

        // 측정은 예고 오프셋 0(예고가 노트 시각에 정확히 완료) 상태에서.
        engine.previewOffset = 0f;

        SongData chart = BuildCalibrationChart();
        engine.LoadSong(chart, forCalibration: true);
        // Classic의 기존 보정 방식은 유지. 새 모드는 실제 플레이 예고 시간을 사용.
        engine.previewWindow = RhythmTestModes.CalibrationPreview(activeMode, previewBeforeCalibration);

        calibrationClip = BuildSilentClip(CALIB_FIRST + CALIB_INTERVAL_BEATS * CalibrationInterval + 2f);
        engine.StartGameWithClip(calibrationClip, countdownSeconds);

        // 타이밍 조정 중엔 뒤의 노트(채워짐)를 봐야 하므로 검정 배경은 끄고 텍스트만 남긴다.
        SetMenuActive(true);
        SetMenuBackground(false);
        RenderCalibText();
    }

    void UpdateCalibration()
    {
        if (calibFinalizing) return;   // 종료 메시지 표시 중 → 입력 무시

        // 예고(채워짐)에 맞춰 노트를 치면 부호 있는 오차를 표본으로 수집
        for (int i = 0; i < engine.LaneCount; i++)
        {
            if (!IsLaneKeyDown(i)) continue;
            RecordCalibrationTap(i);
            if (calibFinalizing) return;
        }
    }

    void RecordCalibrationTap(int lane)
    {
        if (!calibrating || calibFinalizing) return;
        if (SfxPlayer.Instance != null) SfxPlayer.Instance.PlayClap();
        if (!engine.TryMeasureTap(lane, out float error)) return;
        calibSamples.Add(error);
        RenderCalibText();
        if (calibSamples.Count >= CALIB_TARGET) FinalizeCalibration();
    }

    void RenderCalibText()
    {
        if (menuText) menuText.gameObject.SetActive(true);
        WriteMenu(
            "타이밍 조정\n\n" +
            "버튼 안이 가득 찰 때 맞춰\n" +
            $"{InputHint} 를 리듬에 맞게 두드리세요.\n\n" +
            $"입력: {calibSamples.Count} / {CALIB_TARGET}\n\n" +
            "ESC : 취소");
    }

    void FinalizeCalibration()
    {
        if (!calibrating || calibFinalizing) return;
        calibFinalizing = true;      // 메뉴 복귀까지 집계 차단
        engine.Halt();               // 노트 진행 정지(잔여 miss 깜빡임 방지)

        if (calibSamples.Count < CALIB_MIN)
        {
            StartCoroutine(CalibDoneThenMenu("표본이 부족합니다. 다시 시도하세요.", false));
            return;
        }

        float offset = Mathf.Clamp(Median(calibSamples), -0.2f, 0.2f);
        PlayerPrefs.SetFloat(engine.OffsetPreferenceKey, offset);
        PlayerPrefs.Save();

        StartCoroutine(CalibDoneThenMenu(
            $"보정 완료: {OffsetLabel(offset)}\n(예고를 {(offset >= 0 ? "앞당김" : "늦춤")})",
            true));
    }

    IEnumerator CalibDoneThenMenu(string msg, bool success)
    {
        SetMenuActive(true);
        SetMenuBackground(true);   // 완료 메시지를 잘 보이게 배경 복원
        if (menuText) menuText.gameObject.SetActive(true);
        WriteMenu((success ? "" : "<color=#ff6666>") + msg + (success ? "" : "</color>"));
        if (AudioManager.Instance != null) AudioManager.Instance.Stop();
        yield return new WaitForSecondsRealtime(1.4f);
        ReturnToMenu();
    }

    static float Median(List<float> values)
    {
        var v = new List<float>(values);
        v.Sort();
        int n = v.Count;
        return (n % 2 == 1) ? v[n / 2] : 0.5f * (v[n / 2 - 1] + v[n / 2]);
    }

    /// <summary>현재 모드의 레인 수와 속도에 맞는 보정 채보.</summary>
    SongData BuildCalibrationChart()
    {
        var song = new SongData
        {
            source = "",                 // 오디오는 무음 클립으로 대체
            difficulty = "calibration",
            meta = new SongMeta { bpm = 60f / CalibrationInterval, seconds_per_beat = CalibrationInterval },
            notes = new List<NoteData>(),
        };
        for (int i = 0; i < CALIB_INTERVAL_BEATS; i++)
        {
            song.notes.Add(new NoteData
            {
                time = CALIB_FIRST + i * CalibrationInterval,
                lane = i % engine.LaneCount, // 이미 0-based. Single은 모두 0.
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

    public void ReturnToMenu()
    {
        StopAllCoroutines();
        if (AudioManager.Instance != null) AudioManager.Instance.Stop();
        if (calibrating) engine.previewWindow = previewBeforeCalibration;
        if (calibrationClip != null) Destroy(calibrationClip);
        calibrationClip = null;
        engine.ResetForMenu();
        activeMode = engine.testMode;
        menuInputAfterFrame = Time.frameCount;
        calibFinalizing = false;
        judgmentTimer = 0f;
        calibSamples.Clear();
        ResetCounters(null);
        textOutput.Write("Score", scoreText, "SCORE\n0000000");
        textOutput.Write("Combo", comboText, "");
        textOutput.Write("Judge", judgmentText, "");
        if (resultScreen != null) resultScreen.Hide();
        ShowMenu();
    }
}
