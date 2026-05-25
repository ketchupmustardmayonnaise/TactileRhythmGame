using UnityEngine;
using UnityEngine.UI;

/// <summary>
/// 8점 점자 셀 하나 (2열 × 4행).
/// BrailleCellDisplay가 그리드 형태로 인스턴스화한다.
///
/// 도트 인덱스 레이아웃:
///   0  1
///   2  3
///   4  5
///   6  7
/// </summary>
[RequireComponent(typeof(RectTransform))]
public class BrailleCell : MonoBehaviour
{
    public const int DotCols     = 2;
    public const int DotRows     = 4;
    public const int DotsPerCell = DotCols * DotRows;  // 8

    [HideInInspector] public Color activeColor    = new Color(0.95f, 0.95f, 0.95f);
    [HideInInspector] public Color inactiveColor  = new Color(0.12f, 0.12f, 0.14f);
    [HideInInspector] public Color highlightColor = new Color(0.2f,  0.5f,  1.0f);

    private Image[] dotImages;
    private bool[]  dotState;
    private bool[]  dotHighlight;

    private static Sprite sharedCircleSprite;

    public void Init(float dotDiameter, float spacingX, float spacingY)
    {
        dotImages    = new Image[DotsPerCell];
        dotState     = new bool[DotsPerCell];
        dotHighlight = new bool[DotsPerCell];

        if (sharedCircleSprite == null)
            sharedCircleSprite = BuildCircleSprite(32);

        for (int i = 0; i < DotsPerCell; i++)
        {
            int dr = i / DotCols;
            int dc = i % DotCols;

            var go = new GameObject($"Dot{i}", typeof(RectTransform), typeof(Image));
            go.transform.SetParent(transform, false);

            var rt = go.GetComponent<RectTransform>();
            rt.anchorMin        = rt.anchorMax = new Vector2(0f, 1f);
            rt.pivot            = new Vector2(0.5f, 0.5f);
            rt.sizeDelta        = Vector2.one * dotDiameter;
            rt.anchoredPosition = new Vector2(
                dc * spacingX + spacingX * 0.5f,
                -(dr * spacingY + spacingY * 0.5f)
            );

            var img = go.GetComponent<Image>();
            img.sprite        = sharedCircleSprite;
            img.raycastTarget = false;
            dotImages[i]      = img;
            ApplyColor(i);
        }
    }

    public void SetDot(int idx, bool active)
    {
        if ((uint)idx >= DotsPerCell) return;
        dotState[idx] = active;
        ApplyColor(idx);
    }

    public void SetDotHighlight(int idx, bool highlighted)
    {
        if ((uint)idx >= DotsPerCell) return;
        dotHighlight[idx] = highlighted;
        ApplyColor(idx);
    }

    public void ClearAll()
    {
        for (int i = 0; i < DotsPerCell; i++)
        {
            dotState[i]     = false;
            dotHighlight[i] = false;
            ApplyColor(i);
        }
    }

    void ApplyColor(int i)
    {
        if (dotImages == null || dotImages[i] == null) return;
        dotImages[i].color = dotState[i]
            ? (dotHighlight[i] ? highlightColor : activeColor)
            : inactiveColor;
    }

    static Sprite BuildCircleSprite(int size)
    {
        var tex = new Texture2D(size, size, TextureFormat.RGBA32, false);
        tex.filterMode = FilterMode.Bilinear;
        var px = new Color32[size * size];
        float c = (size - 1) * 0.5f;
        float r = size * 0.5f - 0.5f;

        for (int y = 0; y < size; y++)
        {
            for (int x = 0; x < size; x++)
            {
                float d = Mathf.Sqrt((x - c) * (x - c) + (y - c) * (y - c));
                byte  a = (byte)(Mathf.Clamp01(r - d + 1f) * 255);
                px[y * size + x] = new Color32(255, 255, 255, a);
            }
        }

        tex.SetPixels32(px);
        tex.Apply();
        return Sprite.Create(tex, new Rect(0, 0, size, size), Vector2.one * 0.5f);
    }
}
