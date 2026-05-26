using UnityEngine;

/// <summary>
/// 점자 디스플레이 위에 렌더링되는 원형 버튼 하나.
/// 위치·크기는 모두 그리드 대비 비율(0~1)로 저장하므로
/// dotRows / dotColumns가 바뀌어도 같은 비율로 배치된다.
/// </summary>
[System.Serializable]
public class BrailleCircleButton
{
    [Tooltip("행 방향 위치 비율 (0 = 맨 위, 1 = 맨 아래)")]
    [Range(0f, 1f)] public float rowRatio;
    [Tooltip("열 방향 위치 비율 (0 = 왼쪽, 1 = 오른쪽)")]
    [Range(0f, 1f)] public float colRatio;
    [Tooltip("반지름 (그리드 행 높이 기준 비율)")]
    [Range(0f, 0.5f)] public float radiusRatio = 0.081f;
    [Tooltip("테두리 두께 (반지름 대비 비율)")]
    [Range(0f, 1f)] public float thicknessRatio = 0.46f;

    public BrailleCircleButton(float rowRatio, float colRatio,
                               float radiusRatio = 0.081f, float thicknessRatio = 0.46f)
    {
        this.rowRatio       = rowRatio;
        this.colRatio       = colRatio;
        this.radiusRatio    = radiusRatio;
        this.thicknessRatio = thicknessRatio;
    }

    /// <summary>아이들 화면용 — activation=1로 테두리를 그린다.</summary>
    public void Draw(BrailleCellDisplay display) => DrawWithActivation(display, 1f);

    /// <summary>노트 예고 — activation(0~1) 값으로 테두리 밝기를 조절한다.</summary>
    public void DrawWithActivation(BrailleCellDisplay display, float t)
    {
        ComputeGeometry(display, out float cr, out float cc,
                        out float radius, out float thickness, out float ax, out float colR);
        ComputeBounds(display, cr, cc, radius, colR,
                      out int rMin, out int rMax, out int cMin, out int cMax);

        for (int r = rMin; r <= rMax; r++)
            for (int c = cMin; c <= cMax; c++)
            {
                float dr   = r - cr;
                float dc   = (c - cc) * ax;
                float dist = Mathf.Sqrt(dr * dr + dc * dc);
                if (Mathf.Abs(dist - radius) <= thickness)
                    display.SetDotActivation(r, c, t);
            }
    }

    /// <summary>셀(row, col)이 이 버튼의 원 내부에 있는지 (터치 판정용).</summary>
    public bool Contains(int row, int col, BrailleCellDisplay display)
    {
        ComputeGeometry(display, out float cr, out float cc,
                        out float radius, out _, out float ax, out _);
        float dr = row - cr;
        float dc = (col - cc) * ax;
        return Mathf.Sqrt(dr * dr + dc * dc) <= radius + 0.5f;
    }

    /// <summary>원 내부 + 테두리 전체를 파란색 하이라이트 on/off.</summary>
    public void SetHighlight(BrailleCellDisplay display, bool on)
    {
        ComputeGeometry(display, out float cr, out float cc,
                        out float radius, out float thickness, out float ax, out float colR);
        ComputeBounds(display, cr, cc, radius, colR,
                      out int rMin, out int rMax, out int cMin, out int cMax);

        for (int r = rMin; r <= rMax; r++)
            for (int c = cMin; c <= cMax; c++)
            {
                float dr   = r - cr;
                float dc   = (c - cc) * ax;
                float dist = Mathf.Sqrt(dr * dr + dc * dc);
                if (dist <= radius + thickness * 0.5f)
                    display.SetDotHighlight(r, c, on);
            }
    }

    // ── geometry helpers ──────────────────────────────────────────────────────

    void ComputeGeometry(BrailleCellDisplay display,
                         out float cr, out float cc,
                         out float radius, out float thickness,
                         out float ax, out float colR)
    {
        cr        = rowRatio        * display.Rows;
        cc        = colRatio        * display.Columns;
        radius    = radiusRatio     * display.Rows;
        thickness = thicknessRatio  * radius;
        ax        = Aspect(display);
        colR      = radius / ax;
    }

    static void ComputeBounds(BrailleCellDisplay display,
                              float cr, float cc, float radius, float colR,
                              out int rMin, out int rMax, out int cMin, out int cMax)
    {
        rMin = Mathf.Max(0,                   Mathf.FloorToInt(cr - radius - 1));
        rMax = Mathf.Min(display.Rows - 1,    Mathf.CeilToInt (cr + radius + 1));
        cMin = Mathf.Max(0,                   Mathf.FloorToInt(cc - colR   - 1));
        cMax = Mathf.Min(display.Columns - 1, Mathf.CeilToInt (cc + colR   + 1));
    }

    static float Aspect(BrailleCellDisplay display)
    {
        var rect = display.GetComponent<RectTransform>().rect;
        if (rect.width <= 0 || rect.height <= 0) return 1f;
        return (rect.width / display.Columns) / (rect.height / display.Rows);
    }
}
