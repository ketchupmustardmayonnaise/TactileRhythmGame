using UnityEngine;
using UnityEngine.UI;
using TMPro;

/// <summary>
/// 게임 씬을 총괄한다.
/// - 씬에 GameEngine, AudioManager, DotMatrixDisplay가 있어야 한다.
/// - laneButtons 배열에 레인 수만큼 Button을 할당한다.
/// - 에디터 테스트: D/F/J/K 키로 레인 0~3 탭
/// </summary>
public class GameScreen : MonoBehaviour
{
    [Header("Core")]
    public GameEngine engine;

    [Header("Lane Input Buttons (UI)")]
    public Button[] laneButtons;

    [Header("HUD (선택)")]
    public TextMeshProUGUI scoreText;
    public TextMeshProUGUI comboText;
    public TextMeshProUGUI judgmentText;

    [Tooltip("Resources/Songs/ 안의 JSON 파일명 (확장자 제외)")]
    public string songResourceName = "sample";

    private float judgmentTimer;
    private static readonly KeyCode[] DebugKeys = { KeyCode.D, KeyCode.F, KeyCode.J, KeyCode.K };

    void Start()
    {
        SongData song = SongLoader.LoadFromResources(songResourceName);
        if (song == null) return;

        engine.LoadSong(song);

        for (int i = 0; i < laneButtons.Length; i++)
        {
            int lane = i;
            laneButtons[i].onClick.AddListener(() => HandleTap(lane));
        }

        engine.StartGame(countdownSeconds: 3f);
    }

    void Update()
    {
        // HUD 갱신
        if (scoreText)   scoreText.text   = $"SCORE\n{engine.Score:D7}";
        if (comboText)   comboText.text   = engine.Combo > 1 ? $"{engine.Combo} COMBO" : "";
        if (judgmentText && judgmentTimer > 0f)
        {
            judgmentTimer -= Time.deltaTime;
            if (judgmentTimer <= 0f) judgmentText.text = "";
        }

        // 에디터 키보드 테스트
#if UNITY_EDITOR
        for (int i = 0; i < Mathf.Min(DebugKeys.Length, laneButtons.Length); i++)
            if (Input.GetKeyDown(DebugKeys[i])) HandleTap(i);
#endif
    }

    void HandleTap(int lane)
    {
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
