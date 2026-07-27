using System.Collections;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.UI;
using TMPro;

/// <summary>
/// SheetMusicData를 5선보(오선지)로 그린다.
/// measuresPerSystem(기본 2) 마디마다 줄을 바꿔서 여러 단(system)으로 나눠 표시한다.
/// [ExecuteAlways]: Play를 누르지 않아도 Scene/Game 뷰에서 바로 악보가 보이도록 에디터에서도 실행된다.
/// </summary>
[ExecuteAlways]
[RequireComponent(typeof(Image))]
public class StaffView : MonoBehaviour
{
    [Header("데이터")]
    [Tooltip("Resources/Songs/ 안의 5선보 JSON 파일명 (확장자 제외)")]
    public string songResourceName = "santoki_sheet";

    [Header("레이아웃")]
    [Tooltip("한 단(system)에 표시할 마디 수")]
    public int measuresPerSystem = 2;
    public float staffLineSpacing = 20f;
    public float systemHeight     = 200f;
    public float systemSpacing    = 40f;
    public float marginTop        = 24f;
    public float marginBottom     = 24f;
    public float marginLeft       = 24f;
    public float marginRight      = 24f;

    [Header("색상")]
    public Color paperColor    = new Color(0.98f, 0.98f, 0.96f);
    public Color staffLineColor = new Color(0.15f, 0.15f, 0.18f);
    public Color noteColor      = new Color(0.1f,  0.1f,  0.12f);

    public SheetMusicData Data { get; private set; }

    private float staffBottomY; // line1(E4)의 system 내 로컬 Y (system은 pivot 상단 기준)
    private const float StemLength      = 60f;
    private const float StemThickness   = 2.4f;
    private const float BeamThickness   = 6f;
    private const float BarlineThickness      = 2f;
    private const float FinalBarlineThickness = 5f;

    private static readonly Dictionary<char, int> LetterIndex = new Dictionary<char, int>
    {
        { 'C', 0 }, { 'D', 1 }, { 'E', 2 }, { 'F', 3 }, { 'G', 4 }, { 'A', 5 }, { 'B', 6 }
    };

    private static Sprite filledHeadSprite;
    private static Sprite hollowHeadSprite;
    private static Sprite flagSprite;

    // --- Unity 라이프사이클 ---

    void OnEnable()
    {
        EnsureLayoutContract();

        if (Application.isPlaying)
            StartCoroutine(BuildNextFrame());
        else
            Build(); // 에디터: 레이아웃이 이미 확정되어 있으므로 바로 빌드
    }

    // 이 뷰는 가로로 꽉 차고(anchor stretch) 세로 높이는 콘텐츠 양에 맞춰 스스로 늘어나는
    // 레이아웃을 전제로 한다. Awake()는 도메인 리로드(스크립트 재컴파일) 후 이미 살아있던
    // 오브젝트에는 다시 호출되지 않으므로, 매번 확실히 재적용되는 OnEnable()에서 강제한다.
    void EnsureLayoutContract()
    {
        var rt = GetComponent<RectTransform>();
        rt.anchorMin     = new Vector2(0f, 1f);
        rt.anchorMax     = new Vector2(1f, 1f);
        rt.pivot         = new Vector2(0.5f, 1f);
        rt.localScale    = Vector3.one;
        rt.localPosition = new Vector3(rt.localPosition.x, rt.localPosition.y, 0f);
    }

    IEnumerator BuildNextFrame()
    {
        yield return null;
        Build();
    }

    void OnRectTransformDimensionsChange()
    {
        if (!Application.isPlaying && isActiveAndEnabled)
            Build();
    }

    /// <summary>외부에서 다른 악보 데이터를 직접 주입하고 싶을 때 사용</summary>
    public void SetData(SheetMusicData data)
    {
        Data = data;
        Build();
    }

    // --- 빌드 ---

    public void Build()
    {
        ClearChildren();
        EnsureSprites();

        if (Data == null)
            Data = SheetMusicLoader.LoadFromResources(songResourceName);

        var bg = GetComponent<Image>();
        if (bg != null) bg.color = paperColor;

        if (Data == null || Data.measures == null || Data.measures.Count == 0)
            return;

        staffBottomY = -(systemHeight - 60f); // 아래쪽에 덧줄/기둥용 여백 60px 확보

        RectTransform rt = GetComponent<RectTransform>();
        float usableWidth = Mathf.Max(0f, rt.rect.width - marginLeft - marginRight);

        int totalMeasures = Data.measures.Count;
        int perSystem     = Mathf.Max(1, measuresPerSystem);
        int systemCount   = Mathf.CeilToInt(totalMeasures / (float)perSystem);

        for (int s = 0; s < systemCount; s++)
        {
            var systemGo = new GameObject($"System_{s}", typeof(RectTransform));
            systemGo.transform.SetParent(transform, false);
            var systemRt = systemGo.GetComponent<RectTransform>();
            systemRt.anchorMin = systemRt.anchorMax = new Vector2(0f, 1f);
            systemRt.pivot     = new Vector2(0f, 1f);
            systemRt.anchoredPosition = new Vector2(marginLeft, -marginTop - s * (systemHeight + systemSpacing));
            systemRt.sizeDelta = new Vector2(usableWidth, systemHeight);

            DrawStaffLines(systemRt, usableWidth);

            int startIdx = s * perSystem;
            int endIdx   = Mathf.Min(startIdx + perSystem, totalMeasures);

            float leftGutter = (s == 0) ? DrawClefAndTimeSignature(systemRt) : 0f;
            float measureAreaWidth = usableWidth - leftGutter;
            float perMeasureWidth  = measureAreaWidth / perSystem; // 마지막 단이 마디 수가 모자라도 폭은 동일하게 유지

            float cursorX = leftGutter;
            for (int m = startIdx; m < endIdx; m++)
            {
                DrawBarline(systemRt, cursorX, thick: false);
                DrawMeasureNotes(systemRt, Data.measures[m], cursorX, perMeasureWidth);
                cursorX += perMeasureWidth;
            }
            DrawBarline(systemRt, cursorX, thick: (s == systemCount - 1));
        }

        // 콘텐츠 높이에 맞춰 이 뷰 자체의 높이를 갱신 (top-anchor + 수동 sizeDelta.y 전제)
        float totalHeight = marginTop + systemCount * systemHeight
                           + Mathf.Max(0, systemCount - 1) * systemSpacing + marginBottom;
        rt.sizeDelta = new Vector2(rt.sizeDelta.x, totalHeight);
    }

    void ClearChildren()
    {
        for (int i = transform.childCount - 1; i >= 0; i--)
        {
            var child = transform.GetChild(i).gameObject;
            if (Application.isPlaying) Destroy(child);
            else DestroyImmediate(child);
        }
    }

    // --- 오선/보표 기호 ---

    void DrawStaffLines(RectTransform systemRt, float width)
    {
        for (int rel = 0; rel <= 8; rel += 2) // E4,G4,B4,D5,F5 (아래→위)
        {
            var go = CreateImage(systemRt, $"StaffLine_{rel}", null, staffLineColor);
            var lrt = go.GetComponent<RectTransform>();
            lrt.anchorMin = lrt.anchorMax = new Vector2(0f, 1f);
            lrt.pivot     = new Vector2(0f, 0.5f);
            lrt.anchoredPosition = new Vector2(0f, YForRelStep(rel));
            lrt.sizeDelta = new Vector2(width, 2f);
        }
    }

    /// <summary>박자표(예: 3/4)를 그리고, 다음 요소들이 시작할 x 오프셋(왼쪽 여백 폭)을 반환한다.</summary>
    float DrawClefAndTimeSignature(RectTransform systemRt)
    {
        const float gutter = 56f;
        float midY = YForRelStep(4); // 가운데 줄(B4) 기준

        CreateTimeSigDigit(systemRt, "TimeSigTop", Data.timeSignature.beatsPerMeasure, gutter, midY + staffLineSpacing);
        CreateTimeSigDigit(systemRt, "TimeSigBottom", Data.timeSignature.beatUnit, gutter, midY - staffLineSpacing);

        return gutter;
    }

    void CreateTimeSigDigit(RectTransform parent, string name, int value, float gutter, float y)
    {
        var go = new GameObject(name, typeof(RectTransform));
        go.transform.SetParent(parent, false);
        var tmp = go.AddComponent<TextMeshProUGUI>();
        tmp.text      = value.ToString();
        tmp.fontSize  = staffLineSpacing * 1.6f;
        tmp.alignment = TextAlignmentOptions.Center;
        tmp.color     = noteColor;
        var trt = go.GetComponent<RectTransform>();
        trt.anchorMin = trt.anchorMax = new Vector2(0f, 1f);
        trt.pivot     = new Vector2(0.5f, 0.5f);
        trt.anchoredPosition = new Vector2(gutter * 0.5f, y);
        trt.sizeDelta = new Vector2(gutter, staffLineSpacing * 1.8f);
    }

    void DrawBarline(RectTransform systemRt, float x, bool thick)
    {
        var go = CreateImage(systemRt, thick ? "FinalBarline" : "Barline", null, staffLineColor);
        var brt = go.GetComponent<RectTransform>();
        brt.anchorMin = brt.anchorMax = new Vector2(0f, 1f);
        brt.pivot     = new Vector2(0.5f, 1f);
        brt.anchoredPosition = new Vector2(x, YForRelStep(8));
        brt.sizeDelta = new Vector2(thick ? FinalBarlineThickness : BarlineThickness, 4f * staffLineSpacing);
    }

    // --- 음표 ---

    void DrawMeasureNotes(RectTransform systemRt, Measure measure, float measureStartX, float measureWidth)
    {
        var notes = measure.notes;
        if (notes == null || notes.Count == 0) return;

        int beatsPerMeasure = Mathf.Max(1, Data.timeSignature.beatsPerMeasure);
        float padding    = measureWidth * 0.1f;
        float innerWidth = measureWidth - 2f * padding;

        var noteX = new float[notes.Count];
        float cursorBeat = 0f;
        for (int i = 0; i < notes.Count; i++)
        {
            float slotWidth = (notes[i].beats / beatsPerMeasure) * innerWidth;
            noteX[i] = measureStartX + padding + (cursorBeat / beatsPerMeasure) * innerWidth + slotWidth * 0.5f;
            cursorBeat += notes[i].beats;
        }

        // 붙임(beam): 연속된 8분음표 이하는 깃발 대신 굵은 빔으로 이어 그린다
        var beamed = new bool[notes.Count];
        for (int i = 0; i < notes.Count - 1; i++)
        {
            if (!beamed[i] && IsEighthOrShorter(notes[i].duration) && IsEighthOrShorter(notes[i + 1].duration))
            {
                beamed[i] = beamed[i + 1] = true;
                DrawBeam(systemRt, notes[i], noteX[i], notes[i + 1], noteX[i + 1]);
            }
        }

        for (int i = 0; i < notes.Count; i++)
        {
            bool needsFlag = IsEighthOrShorter(notes[i].duration) && !beamed[i];
            DrawNote(systemRt, notes[i], noteX[i], drawStemAndFlag: !beamed[i], drawFlag: needsFlag);
        }
    }

    void DrawNote(RectTransform systemRt, SheetNote note, float x, bool drawStemAndFlag, bool drawFlag)
    {
        int relStep = RelativeStep(note.pitch);
        float y = YForRelStep(relStep);
        bool stemUp = relStep < 4; // 가운데 줄(B4) 아래면 기둥 위로, 위면 기둥 아래로
        bool isOpenHead = note.duration == "half" || note.duration == "dotted-half" || note.duration == "whole";
        bool isDotted   = note.duration.StartsWith("dotted");

        DrawLedgerLinesIfNeeded(systemRt, relStep, x);

        float headW = staffLineSpacing * 1.15f;
        float headH = staffLineSpacing * 0.85f;
        var headGo = CreateImage(systemRt, "NoteHead", isOpenHead ? hollowHeadSprite : filledHeadSprite, noteColor);
        var headRt = headGo.GetComponent<RectTransform>();
        headRt.anchorMin = headRt.anchorMax = new Vector2(0f, 1f);
        headRt.pivot     = new Vector2(0.5f, 0.5f);
        headRt.anchoredPosition = new Vector2(x, y);
        headRt.sizeDelta = new Vector2(headW, headH);

        if (drawStemAndFlag && note.duration != "whole")
        {
            float stemX  = x + (stemUp ? headW * 0.42f : -headW * 0.42f);
            float tipY   = stemUp ? y + StemLength : y - StemLength;
            DrawStemLine(systemRt, stemX, y, tipY);

            if (drawFlag)
            {
                var flagGo = CreateImage(systemRt, "Flag", flagSprite, noteColor);
                var flagRt = flagGo.GetComponent<RectTransform>();
                flagRt.anchorMin = flagRt.anchorMax = new Vector2(0f, 1f);
                flagRt.pivot     = new Vector2(0f, stemUp ? 1f : 0f);
                flagRt.anchoredPosition = new Vector2(stemX, tipY);
                flagRt.sizeDelta = new Vector2(staffLineSpacing, staffLineSpacing * 1.4f);
                flagRt.localScale = stemUp ? Vector3.one : new Vector3(1f, -1f, 1f);
            }
        }

        if (isDotted)
        {
            var dotGo = CreateImage(systemRt, "Dot", filledHeadSprite, noteColor);
            var dotRt = dotGo.GetComponent<RectTransform>();
            dotRt.anchorMin = dotRt.anchorMax = new Vector2(0f, 1f);
            dotRt.pivot     = new Vector2(0.5f, 0.5f);
            dotRt.anchoredPosition = new Vector2(x + headW * 0.85f, y);
            dotRt.sizeDelta = Vector2.one * (headH * 0.3f);
        }
    }

    void DrawBeam(RectTransform systemRt, SheetNote n1, float x1, SheetNote n2, float x2)
    {
        int r1 = RelativeStep(n1.pitch);
        int r2 = RelativeStep(n2.pitch);
        bool stemUp = (r1 + r2) < 8; // 두 음의 평균 위치로 방향 통일

        float y1 = YForRelStep(r1);
        float y2 = YForRelStep(r2);
        float headW = staffLineSpacing * 1.15f;
        float stemX1 = x1 + (stemUp ? headW * 0.42f : -headW * 0.42f);
        float stemX2 = x2 + (stemUp ? headW * 0.42f : -headW * 0.42f);
        float tipY1  = stemUp ? y1 + StemLength : y1 - StemLength;
        float tipY2  = stemUp ? y2 + StemLength : y2 - StemLength;

        DrawStemLine(systemRt, stemX1, y1, tipY1);
        DrawStemLine(systemRt, stemX2, y2, tipY2);

        var beamGo = CreateImage(systemRt, "Beam", null, noteColor);
        var brt = beamGo.GetComponent<RectTransform>();
        brt.anchorMin = brt.anchorMax = new Vector2(0f, 1f);
        brt.pivot     = new Vector2(0.5f, 0.5f);
        Vector2 p1 = new Vector2(stemX1, tipY1);
        Vector2 p2 = new Vector2(stemX2, tipY2);
        brt.anchoredPosition = (p1 + p2) * 0.5f;
        brt.sizeDelta   = new Vector2(Vector2.Distance(p1, p2), BeamThickness);
        brt.localRotation = Quaternion.Euler(0f, 0f, Mathf.Atan2(p2.y - p1.y, p2.x - p1.x) * Mathf.Rad2Deg);
    }

    void DrawStemLine(RectTransform systemRt, float x, float fromY, float toY)
    {
        var go = CreateImage(systemRt, "Stem", null, noteColor);
        var srt = go.GetComponent<RectTransform>();
        srt.anchorMin = srt.anchorMax = new Vector2(0f, 1f);
        srt.pivot     = new Vector2(0.5f, 0.5f);
        srt.anchoredPosition = new Vector2(x, (fromY + toY) * 0.5f);
        srt.sizeDelta = new Vector2(StemThickness, Mathf.Abs(toY - fromY));
    }

    void DrawLedgerLinesIfNeeded(RectTransform systemRt, int relStep, float x)
    {
        if (relStep >= 0 && relStep <= 8) return; // 오선 안이면 덧줄 불필요

        float width = staffLineSpacing * 1.8f;
        if (relStep > 8)
        {
            for (int step = 10; step <= relStep; step += 2)
                CreateLedgerLine(systemRt, x, YForRelStep(step), width);
        }
        else
        {
            for (int step = -2; step >= relStep; step -= 2)
                CreateLedgerLine(systemRt, x, YForRelStep(step), width);
        }
    }

    void CreateLedgerLine(RectTransform systemRt, float x, float y, float width)
    {
        var go = CreateImage(systemRt, "Ledger", null, staffLineColor);
        var lrt = go.GetComponent<RectTransform>();
        lrt.anchorMin = lrt.anchorMax = new Vector2(0f, 1f);
        lrt.pivot     = new Vector2(0.5f, 0.5f);
        lrt.anchoredPosition = new Vector2(x, y);
        lrt.sizeDelta = new Vector2(width, 2f);
    }

    // --- 유틸 ---

    /// <summary>rel=0(E4, 1번째 줄) 기준 상대 계단 수 → system 내 로컬 Y. 한 칸(줄과 칸 사이)은 staffLineSpacing/2.</summary>
    float YForRelStep(int relStep) => staffBottomY + relStep * (staffLineSpacing * 0.5f);

    static int AbsoluteDiatonic(string pitch)
    {
        char letter = pitch[0];
        int octave  = int.Parse(pitch.Substring(1));
        return octave * 7 + LetterIndex[letter];
    }

    static int RelativeStep(string pitch) => AbsoluteDiatonic(pitch) - AbsoluteDiatonic("E4");

    static bool IsEighthOrShorter(string duration) =>
        duration == "eighth" || duration == "dotted-eighth" || duration == "sixteenth";

    GameObject CreateImage(Transform parent, string name, Sprite sprite, Color color)
    {
        var go = new GameObject(name, typeof(RectTransform), typeof(CanvasRenderer), typeof(Image));
        go.transform.SetParent(parent, false);
        var img = go.GetComponent<Image>();
        img.sprite = sprite;
        img.color  = color;
        img.raycastTarget = false;
        return go;
    }

    // --- 절차적 스프라이트 (음표머리 / 깃발) ---

    static void EnsureSprites()
    {
        if (filledHeadSprite == null) filledHeadSprite = BuildNoteHeadSprite(48, 36, filled: true);
        if (hollowHeadSprite == null) hollowHeadSprite = BuildNoteHeadSprite(48, 36, filled: false);
        if (flagSprite == null)       flagSprite       = BuildFlagSprite(24, 32);
    }

    static Sprite BuildNoteHeadSprite(int w, int h, bool filled)
    {
        var tex = new Texture2D(w, h, TextureFormat.RGBA32, false);
        tex.filterMode = FilterMode.Bilinear;
        var px = new Color32[w * h];
        float cx = (w - 1) * 0.5f, cy = (h - 1) * 0.5f;
        float rx = w * 0.5f - 0.5f, ry = h * 0.5f - 0.5f;
        float ringThicknessPx = Mathf.Min(rx, ry) * 0.24f;

        for (int y = 0; y < h; y++)
        {
            for (int x = 0; x < w; x++)
            {
                float nx = (x - cx) / rx, ny = (y - cy) / ry;
                float d  = Mathf.Sqrt(nx * nx + ny * ny); // 1.0 = 타원 경계
                byte a;
                if (filled)
                {
                    a = (byte)(Mathf.Clamp01((1f - d) * Mathf.Min(rx, ry) + 1f) * 255);
                }
                else
                {
                    float distFromEdgePx = Mathf.Abs(d - 1f) * Mathf.Min(rx, ry);
                    a = d <= 1.5f ? (byte)(Mathf.Clamp01(ringThicknessPx - distFromEdgePx + 1f) * 255) : (byte)0;
                }
                px[y * w + x] = new Color32(255, 255, 255, a);
            }
        }
        tex.SetPixels32(px);
        tex.Apply();
        return Sprite.Create(tex, new Rect(0, 0, w, h), new Vector2(0.5f, 0.5f));
    }

    static Sprite BuildFlagSprite(int w, int h)
    {
        var tex = new Texture2D(w, h, TextureFormat.RGBA32, false);
        tex.filterMode = FilterMode.Bilinear;
        var px = new Color32[w * h];

        for (int y = 0; y < h; y++)
        {
            for (int x = 0; x < w; x++)
            {
                float nx = x / (float)(w - 1);
                float ny = y / (float)(h - 1); // Texture2D: y=0이 아래쪽 행
                float topEdge    = 1f - nx * 0.8f;
                float bottomEdge = 0.5f - nx * 0.5f;
                bool inside = ny <= topEdge && ny >= Mathf.Max(0f, bottomEdge);
                px[y * w + x] = new Color32(255, 255, 255, inside ? (byte)255 : (byte)0);
            }
        }
        tex.SetPixels32(px);
        tex.Apply();
        return Sprite.Create(tex, new Rect(0, 0, w, h), new Vector2(0f, 1f)); // 기둥 끝(좌상단)에 붙이는 기준점
    }
}
