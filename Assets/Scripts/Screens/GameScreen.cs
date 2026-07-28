using UnityEngine;
using UnityEngine.UI;
using UnityEngine.SceneManagement;
using TMPro;

/// <summary>
/// 게임 씬을 총괄한다.
/// - 시작 시 모드 선택 메뉴(2/4/6 key)를 띄우고, 선택된 모드로 채보를 로드한다.
/// - 채보 파일명 규칙: {songResourceName}_2k / _4k / _6k
///   (6key는 접미사 파일이 없으면 기존 무접미사 파일로 폴백)
/// - 곡 전환: 인스펙터의 Song Resource Name 값을 바꾸면 된다(메뉴 없이 수동).
/// - 키 배치: 2key = D/K, 4key = W S / I K, 6key = S D F / J L K  (K↔L 스왑 적용됨)
/// - ESC: 씬을 리로드해 메뉴로 복귀(여러 모드 반복 테스트용).
/// </summary>
public class GameScreen : MonoBehaviour
{
    [Header("Core")]
    public GameEngine engine;

    [Header("HUD (선택)")]
    public TextMeshProUGUI scoreText;
    public TextMeshProUGUI comboText;
    public TextMeshProUGUI judgmentText;

    [Header("Menu (선택)")]
    [Tooltip("모드 선택 안내 텍스트. 없어도 숫자키(2/4/6)로 동작한다.")]
    public TextMeshProUGUI menuText;

    [Tooltip("곡 전환은 여기를 바꾼다. Resources/Songs/ 안 JSON 파일명 접두(확장자·접미사 제외). 예: heavy_serenade")]
    public string songResourceName = "heavy_serenade";

    [Tooltip("카운트다운(초). 0이면 즉시 시작")]
    public float countdownSeconds = 3f;

    // ── 모드별 키 배치 ────────────────────────────────────────────────────────
    //   2key = D/K
    //   4key = W S / I K
    //   6key = S D F / J L K
    private static readonly KeyCode[] Keys2 = { KeyCode.D, KeyCode.K };
    private static readonly KeyCode[] Keys4 = { KeyCode.W, KeyCode.S, KeyCode.I, KeyCode.K };
    private static readonly KeyCode[] Keys6 = { KeyCode.S, KeyCode.D, KeyCode.F,
                                                KeyCode.J, KeyCode.L, KeyCode.K };

    private KeyCode[] activeKeys = Keys6;
    private bool  inMenu = true;
    private float judgmentTimer;

    void Start() => ShowMenu();

    // ── 채보 존재 확인 ────────────────────────────────────────────────────────

    /// <summary>모드에 해당하는 채보 리소스명을 조용히 찾는다(없으면 null, 에러 로그 없음).</summary>
    static string ResolveChart(string baseName, int mode)
    {
        string suffix  = mode == 2 ? "_2k" : mode == 4 ? "_4k" : "_6k";
        string primary = baseName + suffix;

        // Resources.Load 자체는 없을 때 조용히 null을 반환한다(에러 로그 없음).
        if (Resources.Load<TextAsset>($"Songs/{primary}") != null) return primary;

        // 6key만: 접미사 파일이 없으면 기존 무접미사 파일로 폴백
        if (mode == 6 && Resources.Load<TextAsset>($"Songs/{baseName}") != null) return baseName;

        return null;
    }

    // ── 메뉴 ──────────────────────────────────────────────────────────────────

    void ShowMenu(string notice = null)
    {
        inMenu = true;

        if (!menuText)
        {
            if (!string.IsNullOrEmpty(notice)) Debug.LogWarning($"[GameScreen] {notice}");
            return;
        }

        // 각 모드의 채보 유무를 미리 표시
        string Avail(int m) => ResolveChart(songResourceName, m) != null ? "" : "   (채보 없음)";
        string head = string.IsNullOrEmpty(notice) ? "" : $"<color=#ff6666>{notice}</color>\n\n";

        menuText.gameObject.SetActive(true);
        menuText.text =
            head +
            "MODE SELECT\n\n" +
            $"곡: {songResourceName}\n\n" +
            $"[2]  2-Key    D  /  K{Avail(2)}\n" +
            $"[4]  4-Key    W S  /  I K{Avail(4)}\n" +
            $"[6]  6-Key    S D F  /  J L K{Avail(6)}\n\n" +
            "ESC : 메뉴로 돌아가기";
    }

    /// <summary>모드 선택. UI 버튼 OnClick에 int 인자(2/4/6)로 연결할 수도 있다.</summary>
    public void SelectMode(int mode)
    {
        string chart = ResolveChart(songResourceName, mode);
        if (chart == null)
        {
            ShowMenu($"'{songResourceName}'에 {mode}-Key 채보가 없습니다. 다른 모드를 선택하세요.");
            return;
        }

        SongData song = SongLoader.LoadFromResources(chart);
        if (song == null || string.IsNullOrEmpty(song.source))
        {
            ShowMenu($"'{chart}.json' 로드 실패 — source 필드를 확인하세요.");
            return;
        }

        activeKeys = mode == 2 ? Keys2 : mode == 4 ? Keys4 : Keys6;
        engine.SetKeyMode(mode);
        engine.LoadSong(song);
        engine.StartGame(countdownSeconds);

        inMenu = false;
        if (menuText) menuText.gameObject.SetActive(false);
    }

    // ── 루프 ──────────────────────────────────────────────────────────────────

    void Update()
    {
        if (inMenu)
        {
            if      (Input.GetKeyDown(KeyCode.Alpha2) || Input.GetKeyDown(KeyCode.Keypad2)) SelectMode(2);
            else if (Input.GetKeyDown(KeyCode.Alpha4) || Input.GetKeyDown(KeyCode.Keypad4)) SelectMode(4);
            else if (Input.GetKeyDown(KeyCode.Alpha6) || Input.GetKeyDown(KeyCode.Keypad6)) SelectMode(6);
            return;
        }

        // ESC → 씬 리로드로 메뉴 복귀
        if (Input.GetKeyDown(KeyCode.Escape))
        {
            SceneManager.LoadScene(SceneManager.GetActiveScene().buildIndex);
            return;
        }

        // HUD 갱신
        if (scoreText) scoreText.text = $"SCORE\n{engine.Score:D7}";
        if (comboText) comboText.text = engine.Combo > 1 ? $"{engine.Combo} COMBO" : "";
        if (judgmentText && judgmentTimer > 0f)
        {
            judgmentTimer -= Time.deltaTime;
            if (judgmentTimer <= 0f) judgmentText.text = "";
        }

        // 키보드 입력 (activeKeys[i] → 레인 i)
        for (int i = 0; i < activeKeys.Length && i < engine.LaneCount; i++)
            if (Input.GetKeyDown(activeKeys[i])) HandleTap(i);
    }

    void HandleTap(int lane)
    {
        if (SfxPlayer.Instance != null) SfxPlayer.Instance.PlayClap();
        HitResult result = engine.TapLane(lane);
        switch (result)
        {
            case HitResult.Perfect:
                ShowJudgment("PERFECT", new Color(0.4f, 1f, 1f));
                break;
            case HitResult.Good:
                ShowJudgment("GOOD", new Color(1f, 0.9f, 0.3f));
                break;
        }
    }

    void ShowJudgment(string text, Color color)
    {
        if (!judgmentText) return;
        judgmentText.text  = text;
        judgmentText.color = color;
        judgmentTimer      = 0.5f;
    }
}
