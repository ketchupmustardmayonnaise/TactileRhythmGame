using System.Collections;
using UnityEngine;
using UnityEngine.UI;
#if UNITY_EDITOR
using UnityEditor;
#endif

/// <summary>
/// BrailleCell(점 하나)을 dotRows × dotColumns 그리드로 배치하는 점자 디스플레이.
/// Start() 다음 프레임에 RectTransform 실제 크기를 읽어 점 간격과 지름을 자동 계산하므로
/// 해상도/화면 크기에 무관하게 항상 화면을 꽉 채운다.
/// </summary>
[ExecuteAlways]
[RequireComponent(typeof(Image))]
public class BrailleCellDisplay : MonoBehaviour
{
    [Header("Grid (dot 단위)")]
    public int dotColumns = 40;
    public int dotRows    = 16;

    [Header("Dot 크기 비율 (0~1)")]
    [Range(0.3f, 0.95f)]
    public float dotFillRatio = 0.7f;

    [Header("Colors")]
    public Color activeColor     = new Color(0.95f, 0.95f, 0.95f);
    public Color inactiveColor   = new Color(0.12f, 0.12f, 0.14f);
    public Color backgroundColor = new Color(0.05f, 0.05f, 0.07f);
    public Color highlightColor  = new Color(0.2f,  0.5f,  1.0f);

    // GameEngine 호환 프로퍼티
    public int Rows    => dotRows;
    public int Columns => dotColumns;
    public int columns => dotColumns;

    /// <summary>GameEngine이 설정. 터치 시 버튼 단위 하이라이트에 사용.</summary>
    [HideInInspector] public BrailleCircleButton[] buttons;

    private BrailleCell[,] cells;
    private Canvas          _canvas;

    void Awake() => _canvas = GetComponentInParent<Canvas>();

    void OnEnable()
    {
#if UNITY_EDITOR
        // 에디터 모드: Canvas 레이아웃이 이미 확정돼 있으므로 바로 빌드
        if (!Application.isPlaying)
            Build();
#endif
    }

    System.Collections.IEnumerator Start()
    {
        // 플레이 모드: 첫 프레임에 Canvas가 실제 크기를 계산한 뒤 빌드
        if (!Application.isPlaying) yield break;
        yield return null;
        Build();
    }

#if UNITY_EDITOR
    void OnValidate()
    {
        // 인스펙터 값 변경 시 에디터에서 즉시 리빌드
        EditorApplication.delayCall += () => { if (this != null) Build(); };
    }
#endif

    public void Build()
    {
        for (int i = transform.childCount - 1; i >= 0; i--)
        {
            var child = transform.GetChild(i).gameObject;
            if (Application.isPlaying) Destroy(child);
            else DestroyImmediate(child);
        }

        cells = new BrailleCell[dotRows, dotColumns];

        var bg = GetComponent<Image>();
        if (bg != null) bg.color = backgroundColor;

        var rect       = GetComponent<RectTransform>().rect;
        float spacingX = rect.width  / dotColumns;
        float spacingY = rect.height / dotRows;
        float diameter = Mathf.Min(spacingX, spacingY) * dotFillRatio;

        for (int r = 0; r < dotRows; r++)
        {
            for (int c = 0; c < dotColumns; c++)
            {
                var go = new GameObject($"Cell_{r}_{c}", typeof(RectTransform), typeof(Image));
                go.transform.SetParent(transform, false);

                var rt = go.GetComponent<RectTransform>();
                rt.anchorMin        = rt.anchorMax = new Vector2(0f, 1f);
                rt.pivot            = new Vector2(0.5f, 0.5f);
                rt.anchoredPosition = new Vector2(
                    (c + 0.5f) * spacingX,
                    -(r + 0.5f) * spacingY
                );
                rt.sizeDelta = Vector2.one * diameter;

                var cell = go.AddComponent<BrailleCell>();
                cell.activeColor    = activeColor;
                cell.inactiveColor  = inactiveColor;
                cell.highlightColor = highlightColor;
                cell.Init(diameter);

                cells[r, c] = cell;
            }
        }
    }

    // 0~1 연속값으로 activation 설정
    public void SetDotActivation(int row, int col, float t)
    {
        if (cells == null || !InBounds(row, col)) return;
        cells[row, col].SetActivation(t);
    }

    // 편의 메서드: bool → SetActivation
    public void SetDot(int row, int col, bool active)
    {
        if (cells == null || !InBounds(row, col)) return;
        cells[row, col].SetActive(active);
    }

    public void SetDotHighlight(int row, int col, bool highlighted)
    {
        if (cells == null || !InBounds(row, col)) return;
        cells[row, col].SetHighlight(highlighted);
    }

    public bool GetDot(int row, int col) => false;

    public void ClearAll()
    {
        if (cells == null) return;
        int rows = cells.GetLength(0);
        int cols = cells.GetLength(1);
        for (int r = 0; r < rows; r++)
            for (int c = 0; c < cols; c++)
                cells[r, c]?.Clear();
    }

    public void Refresh() { }

    // cells 배열의 실제 크기로 검사 — dotRows/dotColumns와 배열이 잠깐 어긋날 때 방어
    bool InBounds(int row, int col) =>
        cells != null &&
        (uint)row < (uint)cells.GetLength(0) &&
        (uint)col < (uint)cells.GetLength(1);

    // --- 터치 하이라이트 ---

    void LateUpdate()
    {
        if (cells == null) return;

        Camera uiCam = _canvas != null ? _canvas.worldCamera : null;

        // 터치된 셀 하나를 추출 (멀티터치 중 첫 번째 유효 터치)
        int tr = -1, tc = -1;
        bool touched = false;

        for (int i = 0; i < Input.touchCount; i++)
        {
            Touch t = Input.GetTouch(i);
            if (t.phase == TouchPhase.Ended || t.phase == TouchPhase.Canceled) continue;
            if (TryGetCellAt(t.position, uiCam, out tr, out tc)) { touched = true; break; }
        }

        if (!touched && Input.GetMouseButton(0))
            touched = TryGetCellAt(Input.mousePosition, uiCam, out tr, out tc);

        if (!touched || !InBounds(tr, tc)) return;

        // 버튼 영역이면 해당 버튼 전체를 하이라이트
        if (buttons != null)
            foreach (var btn in buttons)
                if (btn.Contains(tr, tc, this)) { btn.SetHighlight(this, true); return; }

        // 버튼 바깥 셀은 개별 하이라이트
        cells[tr, tc].SetHighlight(true);
    }

    bool TryGetCellAt(Vector2 screenPos, Camera uiCam, out int row, out int col)
    {
        row = col = 0;
        var rt = GetComponent<RectTransform>();
        if (!RectTransformUtility.ScreenPointToLocalPointInRectangle(
                rt, screenPos, uiCam, out Vector2 local))
            return false;

        Rect rect  = rt.rect;
        float normX = (local.x - rect.xMin) / rect.width;
        float normY = (local.y - rect.yMin) / rect.height; // 0=아래, 1=위

        if (normX < 0f || normX >= 1f || normY < 0f || normY >= 1f)
            return false;

        col = Mathf.FloorToInt(normX * cells.GetLength(1));
        row = Mathf.FloorToInt((1f - normY) * cells.GetLength(0)); // row 0 = 맨 위
        return true;
    }
}
