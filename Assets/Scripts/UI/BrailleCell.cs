using UnityEngine;
using UnityEngine.UI;

/// <summary>
/// 점자 디스플레이의 점 하나.
///
/// 상태:
///   activation (float 0~1) : 0 = inactive(점 내려감), 1 = active(점 완전히 올라감). 연속값.
///   highlighted (bool)     : 터치됨 → highlightColor 고정 표시.
///
/// BrailleCellDisplay가 rows × columns 그리드로 배치한다.
/// </summary>
[RequireComponent(typeof(RectTransform), typeof(Image))]
public class BrailleCell : MonoBehaviour
{
    [HideInInspector] public Color activeColor    = new Color(0.95f, 0.95f, 0.95f);
    [HideInInspector] public Color inactiveColor  = new Color(0.12f, 0.12f, 0.14f);
    [HideInInspector] public Color highlightColor = new Color(0.2f,  0.5f,  1.0f);

    private Image _image;
    private float _activation;   // 0 = inactive, 1 = active
    private bool  _highlighted;

    private static Sprite sharedCircleSprite;

    public void Init(float diameter)
    {
        _image = GetComponent<Image>();

        if (sharedCircleSprite == null)
            sharedCircleSprite = BuildCircleSprite(32);

        _image.sprite        = sharedCircleSprite;
        _image.raycastTarget = false;

        GetComponent<RectTransform>().sizeDelta = Vector2.one * diameter;

        ApplyColor();
    }

    // 0 = inactive, 1 = active (연속값)
    public void SetActivation(float t)
    {
        _activation = Mathf.Clamp01(t);
        ApplyColor();
    }

    // 편의 메서드: bool → SetActivation
    public void SetActive(bool active) => SetActivation(active ? 1f : 0f);

    public void SetHighlight(bool highlighted)
    {
        _highlighted = highlighted;
        ApplyColor();
    }

    public void Clear()
    {
        _activation  = 0f;
        _highlighted = false;
        ApplyColor();
    }

    void ApplyColor()
    {
        if (_image == null) return;
        _image.color = _highlighted
            ? highlightColor
            : Color.Lerp(inactiveColor, activeColor, _activation);
    }

    static Sprite BuildCircleSprite(int size)
    {
        var tex = new Texture2D(size, size, TextureFormat.RGBA32, false);
        tex.filterMode = FilterMode.Bilinear;
        var px = new Color32[size * size];
        float c = (size - 1) * 0.5f;
        float r = size * 0.5f - 0.5f;

        for (int y = 0; y < size; y++)
            for (int x = 0; x < size; x++)
            {
                float d = Mathf.Sqrt((x - c) * (x - c) + (y - c) * (y - c));
                byte  a = (byte)(Mathf.Clamp01(r - d + 1f) * 255);
                px[y * size + x] = new Color32(255, 255, 255, a);
            }

        tex.SetPixels32(px);
        tex.Apply();
        return Sprite.Create(tex, new Rect(0, 0, size, size), Vector2.one * 0.5f);
    }
}
