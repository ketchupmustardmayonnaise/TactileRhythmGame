using UnityEngine;
using UnityEngine.SceneManagement;
using TMPro;

/// <summary>
/// 곡 종료 후 결과 창.
/// 최종 점수 / Perfect·Good·Miss 개수 / 가장 길게 이어진 콤보(Max Combo)를 표시하고,
/// 아무 키(또는 마우스/터치)나 누르면 선택한 테스트 모드를 유지하고 메뉴로 돌아간다.
///
/// ── 에디터 설정 (Canvas 안에 만든다) ──────────────────────────────────────
///  1) Hierarchy에서 Canvas > 우클릭 > Create Empty, 이름을 "ResultPanel" 로.
///     - RectTransform을 화면 전체로: Anchor Presets에서 Alt+Shift 누른 채
///       stretch-stretch(맨 오른쪽 아래) 선택 → Left/Right/Top/Bottom 0.
///  2) ResultPanel에 Image 컴포넌트 추가(반투명 검정 배경 추천: color 알파 ~200/255).
///  3) ResultPanel 자식으로 TextMeshPro - Text(UI) 하나 추가, 이름 "ResultText".
///     - RectTransform stretch-stretch, 여백 40 정도.
///     - Alignment: 가로/세로 모두 Center.  Font Size: 40 내외.  Wrapping: Enabled.
///     - (원하면 제목/본문을 나눠 titleText / bodyText 두 개로 만들어도 됨.)
///  4) 이 스크립트(ResultScreen)를 ResultPanel(또는 아무 오브젝트)에 Add Component.
///     - panelRoot  ← ResultPanel  (자기 자신 GameObject를 넣어도 됨)
///     - titleText  ← (선택) 제목용 TMP. 없으면 bodyText 한 곳에 다 나온다.
///     - bodyText   ← ResultText
///  5) ResultPanel 을 처음엔 비활성(체크 해제)으로 둔다. 곡이 끝나면 자동으로 켜진다.
///  6) 이 Canvas의 Sort Order가 게임 화면보다 위에 오도록,
///     또는 ResultPanel이 Hierarchy에서 가장 아래(=가장 앞)에 오도록 둔다.
///  7) GameScreen 컴포넌트의 Result Screen 칸에 이 오브젝트를 연결한다.
/// </summary>
public class ResultScreen : MonoBehaviour
{
    public event System.Action MenuRequested;
    public GameTextOutput TextOutput { get; set; }
    private string resultBody = "";
    [Header("Refs")]
    [Tooltip("결과창 전체 루트. 켜고 끄는 대상. 비우면 이 스크립트가 붙은 오브젝트를 사용")]
    public GameObject panelRoot;

    [Tooltip("(선택) 제목 텍스트. 없으면 bodyText 하나에 전부 표시")]
    public TextMeshProUGUI titleText;

    [Tooltip("점수/개수/콤보/안내가 표시되는 본문 텍스트")]
    public TextMeshProUGUI bodyText;

    [Header("Behaviour")]
    [Tooltip("표시 직후 이 시간(초) 동안은 입력을 무시(마지막 노트 입력이 곧바로 넘어가는 것 방지)")]
    public float inputLockSeconds = 0.6f;

    private bool showing;
    private float shownAt;

    /// <summary>결과창이 현재 표시 중인지.</summary>
    public bool IsShowing => showing;

    void Awake()
    {
        if (panelRoot == null) panelRoot = gameObject;
        // 비활성 상태에서 처음 Show를 호출하면 SetActive 중 Awake가 실행될 수 있다.
        panelRoot.SetActive(showing);
    }

    /// <summary>결과를 표시한다. GameScreen이 곡 종료 시 호출.</summary>
    public void Show(int score, int perfect, int good, int miss, int maxCombo)
    {
        showing = true;
        shownAt = Time.unscaledTime;
        if (panelRoot == null) panelRoot = gameObject;
        panelRoot.SetActive(true);
        resultBody =
                $"SCORE   {score:D7}\n\n" +
                $"<color=#66FFFF>PERFECT</color>   {perfect}\n" +
                $"<color=#FFE066>GOOD</color>      {good}\n" +
                $"<color=#FF6666>MISS</color>      {miss}\n\n" +
                $"MAX COMBO   {maxCombo}\n\n" +
                "<size=70%>아무 키나 누르면 메뉴로</size>";
        RefreshPresentation();
    }

    public void RefreshPresentation()
    {
        if (!showing) return;
        if (TextOutput != null)
        {
            if (TextOutput.ConsoleOnly)
            {
                TextOutput.Write("Result", null, "RESULT\n\n" + resultBody);
                // 콘솔에서 UI로 전환해도 최신 결과를 표시한다.
                if (titleText) titleText.text = "RESULT";
                if (bodyText) bodyText.text = (titleText ? "" : "RESULT\n\n") + resultBody;
            }
            else
            {
                TextOutput.Write("ResultTitle", titleText, "RESULT");
                TextOutput.Write("Result", bodyText, (titleText ? "" : "RESULT\n\n") + resultBody);
            }
        }
        else
        {
            if (titleText) titleText.text = "RESULT";
            if (bodyText) bodyText.text = (titleText ? "" : "RESULT\n\n") + resultBody;
        }
    }

    void Update()
    {
        if (!showing) return;
        if (Time.unscaledTime - shownAt < inputLockSeconds) return;

        // 아무 키/마우스/터치 → 메뉴로
        if (Input.anyKeyDown || Input.touchCount > 0)
            ReturnToMenu();
    }

    void ReturnToMenu()
    {
        if (MenuRequested != null)
        {
            MenuRequested.Invoke();
            return;
        }
        showing = false;
        if (AudioManager.Instance != null) AudioManager.Instance.Stop();
        SceneManager.LoadScene(SceneManager.GetActiveScene().buildIndex);
    }

    public void Hide()
    {
        showing = false;
        if (panelRoot != null) panelRoot.SetActive(false);
    }
}
