using UnityEngine;
using UnityEngine.UI;

/// <summary>
/// BrailleCell 오브젝트 그리드로 구성된 점자 디스플레이.
/// DotMatrixDisplay와 동일한 public API를 유지하므로 GameEngine 수정 최소화.
///
/// 그리드 대응:
///   셀 20열 × 4행 = 80 BrailleCell
///   각 셀 2열 × 4행 = 8점
///   총 40열 × 16행 = 640점 (DotMatrixDisplay와 동일)
/// </summary>
[RequireComponent(typeof(Image))]
public class BrailleCellDisplay : MonoBehaviour
{
    [Header("Grid (셀 단위)")]
    public int cellColumns = 20;
    public int cellRows    = 4;

    [Header("Dot/Cell 크기")]
    public float dotDiameter = 12f;
    public float dotSpacingX = 18f;
    public float dotSpacingY = 18f;
    public float cellGapX    = 6f;
    public float cellGapY    = 6f;

    [Header("Colors")]
    public Color activeColor     = new Color(0.95f, 0.95f, 0.95f);
    public Color inactiveColor   = new Color(0.12f, 0.12f, 0.14f);
    public Color backgroundColor = new Color(0.05f, 0.05f, 0.07f);
    public Color highlightColor  = new Color(0.2f,  0.5f,  1.0f);

    // DotMatrixDisplay 호환 프로퍼티
    public int Rows    => cellRows    * BrailleCell.DotRows;   // 16
    public int Columns => cellColumns * BrailleCell.DotCols;   // 40
    public int columns => Columns;

    private BrailleCell[,] cells;

    void Awake() => Build();

    public void Build()
    {
        for (int i = transform.childCount - 1; i >= 0; i--)
        {
            var child = transform.GetChild(i).gameObject;
            if (Application.isPlaying) Destroy(child);
            else DestroyImmediate(child);
        }

        cells = new BrailleCell[cellRows, cellColumns];

        var bg = GetComponent<Image>();
        if (bg != null) bg.color = backgroundColor;

        float cellW = BrailleCell.DotCols * dotSpacingX + cellGapX;
        float cellH = BrailleCell.DotRows * dotSpacingY + cellGapY;

        for (int cr = 0; cr < cellRows; cr++)
        {
            for (int cc = 0; cc < cellColumns; cc++)
            {
                var go = new GameObject($"Cell_{cr}_{cc}", typeof(RectTransform));
                go.transform.SetParent(transform, false);

                var rt = go.GetComponent<RectTransform>();
                rt.anchorMin        = rt.anchorMax = new Vector2(0f, 1f);
                rt.pivot            = new Vector2(0f, 1f);
                rt.anchoredPosition = new Vector2(cc * cellW, -cr * cellH);
                rt.sizeDelta        = new Vector2(cellW, cellH);

                var cell = go.AddComponent<BrailleCell>();
                cell.activeColor    = activeColor;
                cell.inactiveColor  = inactiveColor;
                cell.highlightColor = highlightColor;
                cell.Init(dotDiameter, dotSpacingX, dotSpacingY);

                cells[cr, cc] = cell;
            }
        }
    }

    // --- DotMatrixDisplay 호환 API ---

    public void SetDot(int row, int col, bool active)
    {
        if (!Resolve(row, col, out var cell, out int di)) return;
        cell.SetDot(di, active);
    }

    public void SetDotHighlight(int row, int col, bool highlighted)
    {
        if (!Resolve(row, col, out var cell, out int di)) return;
        cell.SetDotHighlight(di, highlighted);
    }

    public bool GetDot(int row, int col) => false;

    public void ClearAll()
    {
        if (cells == null) return;
        for (int cr = 0; cr < cellRows; cr++)
            for (int cc = 0; cc < cellColumns; cc++)
                cells[cr, cc]?.ClearAll();
    }

    public void Refresh() { }

    // --- 내부 ---

    bool Resolve(int row, int col, out BrailleCell cell, out int dotIdx)
    {
        int cr = row / BrailleCell.DotRows;
        int cc = col / BrailleCell.DotCols;
        int dr = row % BrailleCell.DotRows;
        int dc = col % BrailleCell.DotCols;

        cell   = null;
        dotIdx = 0;

        if ((uint)cr >= (uint)cellRows || (uint)cc >= (uint)cellColumns) return false;
        cell   = cells[cr, cc];
        dotIdx = dr * BrailleCell.DotCols + dc;
        return cell != null;
    }
}
