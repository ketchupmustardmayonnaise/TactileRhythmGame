using System.Collections;
using UnityEngine;
using UnityEngine.UI;

/// <summary>
/// BrailleCell(점 하나)을 dotRows × dotColumns 그리드로 배치하는 점자 디스플레이.
/// Start() 다음 프레임에 RectTransform 실제 크기를 읽어 점 간격과 지름을 자동 계산하므로
/// 해상도/화면 크기에 무관하게 항상 화면을 꽉 채운다.
/// </summary>
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

    private BrailleCell[,] cells;
    private Canvas          _canvas;

    void Awake() => _canvas = GetComponentInParent<Canvas>();

    System.Collections.IEnumerator Start()
    {
        yield return null; // 레이아웃 확정 대기 (Canvas가 실제 크기를 계산한 뒤 실행)
        Build();
    }

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
        for (int r = 0; r < dotRows; r++)
            for (int c = 0; c < dotColumns; c++)
                cells[r, c]?.Clear();
    }

    public void Refresh() { }

    bool InBounds(int row, int col) =>
        (uint)row < (uint)dotRows && (uint)col < (uint)dotColumns;

    // --- 터치 하이라이트 ---

    void LateUpdate()
    {
        if (cells == null) return;

        Camera uiCam = _canvas != null ? _canvas.worldCamera : null;

        // 모바일 멀티터치
        for (int i = 0; i < Input.touchCount; i++)
        {
            Touch touch = Input.GetTouch(i);
            if (touch.phase == TouchPhase.Ended || touch.phase == TouchPhase.Canceled)
                continue;
            if (TryGetCellAt(touch.position, uiCam, out int r, out int c))
                cells[r, c].SetHighlight(true);
        }

        // PC / 에디터 마우스 (좌클릭)
        if (Input.GetMouseButton(0))
        {
            if (TryGetCellAt(Input.mousePosition, uiCam, out int r, out int c))
                cells[r, c].SetHighlight(true);
        }
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

        col = Mathf.FloorToInt(normX * dotColumns);
        row = Mathf.FloorToInt((1f - normY) * dotRows); // row 0 = 맨 위
        return true;
    }
}
