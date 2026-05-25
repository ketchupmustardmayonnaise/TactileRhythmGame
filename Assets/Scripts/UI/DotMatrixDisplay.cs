using UnityEngine;
using UnityEngine.UI;
#if UNITY_EDITOR
using UnityEditor;
#endif

/// <summary>
/// 점자 스타일 도트 매트릭스 디스플레이.
/// RawImage 컴포넌트에 Texture2D를 직접 그려서 표시한다.
/// [ExecuteAlways] 덕분에 Play 없이 에디터에서도 미리보기가 된다.
/// </summary>
[ExecuteAlways]
[RequireComponent(typeof(RawImage))]
public class DotMatrixDisplay : MonoBehaviour
{
    [Header("Grid")]
    [Tooltip("가로 도트 수 (36~50 권장)")]
    public int columns = 40;
    [Tooltip("세로 도트 수")]
    public int rows = 16;

    [Header("Dot Appearance")]
    [Tooltip("도트 반지름(픽셀)")]
    public float dotRadius = 6f;
    [Tooltip("도트 간격 배율 (dotRadius * spacing)")]
    public float spacingFactor = 2.8f;

    [Header("Colors")]
    public Color activeColor     = new Color(0.95f, 0.95f, 0.95f);
    public Color inactiveColor   = new Color(0.12f, 0.12f, 0.14f);
    public Color backgroundColor = new Color(0.05f, 0.05f, 0.07f);
    public Color highlightColor  = new Color(0.2f,  0.5f,  1.0f);

    // 공개 접근자 — GameEngine이 읽기
    public int Columns => columns;
    public int Rows => rows;

    private bool[,] dotState;
    private bool[,] dotHighlight;
    private Texture2D tex;
    private Color32[] pixels;
    private RawImage rawImage;

    private int texW, texH;
    private float spacing;
    void OnEnable() => Build();

    void OnValidate()
    {
#if UNITY_EDITOR
        // 에디터 모드에서는 다음 에디터 틱에 Build() 호출
        EditorApplication.delayCall -= Build;
        EditorApplication.delayCall += Build;
#endif
    }

    void Build()
    {
        if (columns <= 0 || rows <= 0 || dotRadius <= 0) return;

        rawImage = GetComponent<RawImage>();
        spacing  = dotRadius * spacingFactor;

        texW = Mathf.CeilToInt(spacing * columns + dotRadius * 2f);
        texH = Mathf.CeilToInt(spacing * rows    + dotRadius * 2f);

        if (tex != null && !Application.isPlaying)
            DestroyImmediate(tex);
        else if (tex != null)
            Destroy(tex);

        tex    = new Texture2D(texW, texH, TextureFormat.RGBA32, false);
        tex.filterMode = FilterMode.Bilinear;
        pixels = new Color32[texW * texH];

        dotState      = new bool[rows, columns];
        dotHighlight  = new bool[rows, columns];
        rawImage.texture = tex;

        ClearAll();
        Refresh();
    }

    public void SetDot(int row, int col, bool active)
    {
        if ((uint)row < (uint)rows && (uint)col < (uint)columns)
            dotState[row, col] = active;
    }

    public void SetDotHighlight(int row, int col, bool highlighted)
    {
        if ((uint)row < (uint)rows && (uint)col < (uint)columns)
            dotHighlight[row, col] = highlighted;
    }

    public bool GetDot(int row, int col) => dotState[row, col];

    public void ClearAll()
    {
        for (int r = 0; r < rows; r++)
            for (int c = 0; c < columns; c++)
            {
                dotState[r, c]     = false;
                dotHighlight[r, c] = false;
            }
    }

    /// <summary>
    /// dotState 배열을 텍스처에 반영한다. Update 마지막에 한 번 호출.
    /// </summary>
    public void Refresh()
    {
        Color32 bgC = backgroundColor;

        // 배경 초기화
        for (int i = 0; i < pixels.Length; i++)
            pixels[i] = bgC;

        int r = dotRadius > 0 ? Mathf.CeilToInt(dotRadius) : 1;

        for (int row = 0; row < rows; row++)
        {
            for (int col = 0; col < columns; col++)
            {
                float cx = dotRadius + col * spacing;
                float cy = dotRadius + (rows - 1 - row) * spacing; // Y 반전

                Color32 dotC = dotState[row, col]
                    ? (dotHighlight[row, col] ? (Color32)highlightColor : (Color32)activeColor)
                    : (Color32)inactiveColor;

                int xMin = Mathf.Max(0, (int)(cx - r));
                int xMax = Mathf.Min(texW - 1, (int)(cx + r));
                int yMin = Mathf.Max(0, (int)(cy - r));
                int yMax = Mathf.Min(texH - 1, (int)(cy + r));

                for (int py = yMin; py <= yMax; py++)
                {
                    for (int px = xMin; px <= xMax; px++)
                    {
                        float dx = px - cx, dy = py - cy;
                        float dist = Mathf.Sqrt(dx * dx + dy * dy);
                        if (dist > dotRadius) continue;

                        // 경계 안티앨리어싱
                        float alpha = Mathf.Clamp01(dotRadius - dist);
                        int idx = py * texW + px;
                        pixels[idx] = Color32.Lerp(bgC, dotC, alpha);
                    }
                }
            }
        }

        tex.SetPixels32(pixels);
        tex.Apply(false);
    }
}
