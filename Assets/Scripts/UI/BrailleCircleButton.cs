using UnityEngine;

/// <summary>
/// 점자 디스플레이 위에 렌더링되는 원형 버튼 하나.
/// 위치·크기는 모두 그리드 대비 비율(0~1)로 저장하므로
/// dotRows / dotColumns가 바뀌어도 같은 비율로 배치된다.
///
/// 렌더링은 거리장(distance field) 기반 이진(on/off) 방식이다.
/// 각 셀 중심에서 원 중심까지 거리를 구해 테두리 링 안이면 점을 켠다.
/// 실제 점자 디스플레이처럼 점은 완전히 켜지거나 꺼지며, 그라데이션이 없다.
/// </summary>
[System.Serializable]
public class BrailleCircleButton
{
    [Tooltip("행 방향 위치 비율 (0 = 맨 위, 1 = 맨 아래)")]
    [Range(0f, 1f)] public float rowRatio;
    [Tooltip("열 방향 위치 비율 (0 = 왼쪽, 1 = 오른쪽)")]
    [Range(0f, 1f)] public float colRatio;
    [Tooltip("반지름 (그리드 행 높이 기준 비율)")]
    [Range(0f, 0.5f)] public float radiusRatio = 0.11f;
    [Tooltip("테두리 두께 (반지름 대비 비율)")]
    [Range(0f, 1f)] public float thicknessRatio = 0.28f;

    public BrailleCircleButton(float rowRatio, float colRatio,
                               float radiusRatio = 0.11f, float thicknessRatio = 0.28f)
    {
        this.rowRatio       = rowRatio;
        this.colRatio       = colRatio;
        this.radiusRatio    = radiusRatio;
        this.thicknessRatio = thicknessRatio;
    }

    /// <summary>아이들 화면용 — 테두리(링)를 또렷하게 그린다.</summary>
    public void Draw(BrailleCellDisplay display) => DrawWithActivation(display, 1f);

    /// <summary>테두리 링을 그린다. t ≤ 0 이면 그리지 않는다(이진 on/off, 그라데이션 없음).</summary>
    public void DrawWithActivation(BrailleCellDisplay display, float t)
    {
        if (t <= 0f) return;

        ComputeGeometry(display, out float cr, out float cc,
                        out float radius, out float thickness, out float ax, out float colR);
        ComputeBounds(display, cr, cc, radius, colR,
                      out int rMin, out int rMax, out int cMin, out int cMax);

        for (int r = rMin; r <= rMax; r++)
            for (int c = cMin; c <= cMax; c++)
                if (InRing(Distance(r, c, cr, cc, ax), radius, thickness))
                    display.SetDotActivation(r, c, 1f);
    }

    /// <summary>
    /// 테두리는 항상 켜고, 내부는 interiorT(0~1)에 따라 안쪽 → 바깥쪽으로
    /// 채워지는 원판이 점점 커진다. 각 점은 완전히 켜지거나 꺼진 이진 상태(그라데이션 없음).
    /// </summary>
    public void DrawFill(BrailleCellDisplay display, float interiorT)
    {
        ComputeGeometry(display, out float cr, out float cc,
                        out float radius, out float thickness, out float ax, out float colR);
        ComputeBounds(display, cr, cc, radius, colR,
                      out int rMin, out int rMax, out int cMin, out int cMax);

        float innerEdge  = radius - thickness;                 // 테두리 안쪽 경계
        float fillRadius = innerEdge * Mathf.Clamp01(interiorT); // 채워진 원판 반지름

        for (int r = rMin; r <= rMax; r++)
            for (int c = cMin; c <= cMax; c++)
            {
                float dist = Distance(r, c, cr, cc, ax);
                if (InRing(dist, radius, thickness) || dist <= fillRadius)
                    display.SetDotActivation(r, c, 1f);
            }
    }

    /// <summary>셀(row, col)이 이 버튼의 원 내부에 있는지 (터치 판정용).</summary>
    public bool Contains(int row, int col, BrailleCellDisplay display)
    {
        ComputeGeometry(display, out float cr, out float cc,
                        out float radius, out _, out float ax, out _);
        return Distance(row, col, cr, cc, ax) <= radius + 0.5f;
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
                if (Distance(r, c, cr, cc, ax) <= radius + thickness)
                    display.SetDotHighlight(r, c, on);
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    /// <summary>셀이 테두리 링 안에 있는지 (이진 판정).</summary>
    static bool InRing(float dist, float radius, float thickness) =>
        Mathf.Abs(dist - radius) <= thickness;

    static float Distance(int r, int c, float cr, float cc, float ax)
    {
        float dr = r - cr;
        float dc = (c - cc) * ax;
        return Mathf.Sqrt(dr * dr + dc * dc);
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
        // 거리 계산 범위는 테두리 바깥 AA 폭까지 포함해야 하므로 여유 +2
        rMin = Mathf.Max(0,                   Mathf.FloorToInt(cr - radius - 2));
        rMax = Mathf.Min(display.Rows - 1,    Mathf.CeilToInt (cr + radius + 2));
        cMin = Mathf.Max(0,                   Mathf.FloorToInt(cc - colR   - 2));
        cMax = Mathf.Min(display.Columns - 1, Mathf.CeilToInt (cc + colR   + 2));
    }

    static float Aspect(BrailleCellDisplay display)
    {
        var rect = display.GetComponent<RectTransform>().rect;
        if (rect.width <= 0 || rect.height <= 0) return 1f;
        return (rect.width / display.Columns) / (rect.height / display.Rows);
    }
}
