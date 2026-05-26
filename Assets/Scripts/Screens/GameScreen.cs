using UnityEngine;
using UnityEngine.UI;
using TMPro;

/// <summary>
/// 게임 씬을 총괄한다.
/// - 씬에 GameEngine, AudioManager, BrailleCellDisplay가 있어야 한다.
/// - 에디터 테스트: S/D/F 키 → 왼쪽 Lane 1-3, J/K/L 키 → 오른쪽 Lane 4-6
/// </summary>
public class GameScreen : MonoBehaviour
{
    [Header("Core")]
    public GameEngine engine;

    [Header("HUD (선택)")]
    public TextMeshProUGUI scoreText;
    public TextMeshProUGUI comboText;
    public TextMeshProUGUI judgmentText;

    [Tooltip("Resources/Songs/ 안의 JSON 파일명 (확장자 제외)")]
    public string songResourceName = "heavy_serenade";

    [Tooltip("카운트다운(초). 0이면 즉시 시작")]
    public float countdownSeconds = 3f;

    private float judgmentTimer;

    // 6키: 왼손 S/D/F = Lane 1-3, 오른손 J/K/L = Lane 4-6
    private static readonly KeyCode[] DebugKeys =
    {
        KeyCode.S, KeyCode.D, KeyCode.F,   // 왼쪽 1-3
        KeyCode.J, KeyCode.K, KeyCode.L    // 오른쪽 4-6
    };

    void Start()
    {
        SongData song = SongLoader.LoadFromResources(songResourceName);
        if (song == null) return;

        if (string.IsNullOrEmpty(song.source))
        {
            Debug.LogError($"[GameScreen] '{songResourceName}.json'에 source 필드가 없습니다. " +
                           "인스펙터의 Song Resource Name을 확인하세요. (예: heavy_serenade)");
            return;
        }

        engine.LoadSong(song);
        engine.StartGame(countdownSeconds);
    }

    void Update()
    {
        // HUD 갱신
        if (scoreText) scoreText.text = $"SCORE\n{engine.Score:D7}";
        if (comboText) comboText.text = engine.Combo > 1 ? $"{engine.Combo} COMBO" : "";
        if (judgmentText && judgmentTimer > 0f)
        {
            judgmentTimer -= Time.deltaTime;
            if (judgmentTimer <= 0f) judgmentText.text = "";
        }

        // 키보드 입력 (에디터 + 빌드 공통)
        for (int i = 0; i < Mathf.Min(DebugKeys.Length, engine.LaneCount); i++)
            if (Input.GetKeyDown(DebugKeys[i])) HandleTap(i);
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
