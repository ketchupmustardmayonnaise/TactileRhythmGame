using System.Collections.Generic;
using UnityEngine;

/// <summary>
/// SheetMusicData(오선지 악보)를 BrailleCellDisplay의 점 격자 해상도로 그대로 래스터화해서 보여준다.
/// 실제 점자악보 점형(Braille Music Code)이 아니라, 오선지 그림(줄/마디선/음높이-길이)을
/// 점자 디스플레이의 raised-dot 그래픽으로 옮긴 것이다.
///
/// 한 번에 한 단(system, 기본 2마디)만 표시하고, 디스플레이 맨 아래 navRows행에 이전/다음
/// 세모도 점자로 그려서 좌우 화살표 키 또는 그 세모를 클릭/터치하면 페이지가 넘어간다.
/// GameEngine.Update()가 그리는 유휴 화면(원형 버튼 테두리)은 Update 단계에서 실행되고
/// 이 뷰는 LateUpdate에서 그 뒤에 다시 그리므로, 항상 이 뷰가 최종적으로 화면에 남는다.
/// </summary>
[ExecuteAlways]
[RequireComponent(typeof(BrailleCellDisplay))]
public class BrailleSheetMusicView : MonoBehaviour
{
    [Header("데이터")]
    [Tooltip("Resources/Songs/ 안의 5선보 JSON 파일명 (확장자 제외)")]
    public string songResourceName = "santoki_sheet";
    [Tooltip("한 번에 표시할 마디 수")]
    public int measuresPerSystem = 2;

    [Header("넘기기")]
    public int currentSystem = 0;
    public KeyCode nextKey = KeyCode.RightArrow;
    public KeyCode prevKey = KeyCode.LeftArrow;

    [Header("세로 위치/간격")]
    [Tooltip("맨 위 음(가장 높은 음)이 시작할 행 - 이 값만큼 위쪽 여백을 둔다")]
    public int topMarginRows = 1;
    [Tooltip("음높이 한 칸(계단) 당 몇 행을 쓸지 - 클수록 오선 다섯 줄 사이 간격이 넓어진다")]
    public float rowsPerStep = 1.5f;

    [Header("이전/다음 넘기기 (점자 세모)")]
    [Tooltip("맨 아래에 이전/다음 세모를 그리기 위해 비워 둘 행 수")]
    public int navRows = 3;
    [Tooltip("세모 하나의 가로 폭(열 수) - 3이면 3행×3열의 작은 세모가 된다")]
    public int navTriangleWidth = 3;
    [Tooltip("세모와 디스플레이 좌우 가장자리 사이 여백(열 수)")]
    public int navSideMargin = 3;

    private BrailleCellDisplay display;
    private SheetMusicData data;
    private int contentRows; // navRows를 뺀, 오선지가 실제로 쓰는 행 수

    private static readonly Dictionary<char, int> LetterIndex = new Dictionary<char, int>
    {
        { 'C', 0 }, { 'D', 1 }, { 'E', 2 }, { 'F', 3 }, { 'G', 4 }, { 'A', 5 }, { 'B', 6 }
    };

    int SystemCount => (data == null || data.measures == null || data.measures.Count == 0)
        ? 1
        : Mathf.Max(1, Mathf.CeilToInt(data.measures.Count / (float)Mathf.Max(1, measuresPerSystem)));

    void OnEnable()
    {
        display = GetComponent<BrailleCellDisplay>();
        data = SheetMusicLoader.LoadFromResources(songResourceName);
        TryRender(); // 에디터: Play를 누르지 않아도 바로 보이도록 즉시 한 번 그린다
    }

    void Update()
    {
        if (data == null) return;
        if (Input.GetKeyDown(nextKey)) NextSystem();
        if (Input.GetKeyDown(prevKey)) PrevSystem();

        if (Application.isPlaying && Input.GetMouseButtonDown(0))
            HandleNavClick(Input.mousePosition);
    }

    void HandleNavClick(Vector2 screenPos)
    {
        if (display == null || !display.TryGetCellAt(screenPos, out int row, out int col))
            return;
        if (row < contentRows) return; // 이전/다음 세모 영역(맨 아래 navRows행) 밖이면 무시

        if (col < display.Columns / 2) PrevSystem();
        else NextSystem();
    }

    void LateUpdate()
    {
        // Play 모드에서는 GameEngine.Update()가 매 프레임 유휴 화면을 다시 그리므로,
        // 이 뷰가 항상 마지막에 그려지도록 매 프레임 재적용한다.
        // 에디터(Edit 모드)에서는 경쟁 상대가 없으므로 OnEnable/OnValidate/넘기기 시점에만 그리면 충분하다.
        if (Application.isPlaying)
            TryRender();
    }

    void OnValidate()
    {
        if (!Application.isPlaying) TryRender();
    }

    void TryRender()
    {
        if (display == null || data == null || data.measures == null || data.measures.Count == 0)
            return;
        Render();
    }

    public void NextSystem()
    {
        currentSystem = (currentSystem + 1) % SystemCount;
        if (!Application.isPlaying) TryRender();
    }

    public void PrevSystem()
    {
        currentSystem = (currentSystem - 1 + SystemCount) % SystemCount;
        if (!Application.isPlaying) TryRender();
    }

    /// <summary>지금 화면에 보이는 마디들(startIdx~endIdx)에서만 쓰인 음높이 범위를 구한다.
    /// (곡 전체가 아니라 현재 페이지 기준으로 구해야, 이 페이지에 없는 높은/낮은 음 때문에
    /// 위쪽에 불필요한 빈 공간이 생기지 않는다.)</summary>
    void ComputeLocalPitchRange(int startIdx, int endIdx, out int lo, out int hi)
    {
        lo = 0;
        hi = 8; // 오선 자체(줄 5개, rel 0~8)는 항상 표시 범위에 포함
        for (int m = startIdx; m < endIdx; m++)
        {
            var notes = data.measures[m].notes;
            if (notes == null) continue;
            foreach (var n in notes)
            {
                int rel = RelativeStep(n.pitch);
                lo = Mathf.Min(lo, rel);
                hi = Mathf.Max(hi, rel);
            }
        }
    }

    void Render()
    {
        display.ClearAll();

        int totalRows = Mathf.Max(2, display.Rows - Mathf.Max(0, navRows));
        contentRows = totalRows;
        int totalCols = display.Columns;
        int perSystem = Mathf.Max(1, measuresPerSystem);
        int systemCount = SystemCount;
        currentSystem = ((currentSystem % systemCount) + systemCount) % systemCount;

        int startIdx = currentSystem * perSystem;
        int endIdx   = Mathf.Min(startIdx + perSystem, data.measures.Count);

        // 세로: 지금 보이는 마디들 기준 가장 높은 음을 topMarginRows에 놓고, 음높이 한 칸마다
        // rowsPerStep행씩 내려가며 배치한다 (아래쪽에 남는 칸은 그냥 비워 둔다).
        ComputeLocalPitchRange(startIdx, endIdx, out _, out int localMax);
        int RowFor(int relStep) =>
            Mathf.Clamp(Mathf.RoundToInt(topMarginRows + (localMax - relStep) * rowsPerStep), 0, totalRows - 1);

        // 오선 5줄 (실제 악보처럼 실선이지만, 음표와 구분되도록 50%로 흐릿하게 켠다)
        for (int rel = 0; rel <= 8; rel += 2)
        {
            int row = RowFor(rel);
            for (int c = 0; c < totalCols; c++)
                display.SetDotActivation(row, c, 0.5f);
        }

        // 가로: 마디선(perSystem+1개) 폭을 제외한 나머지를 마디 수만큼 균등 배분
        int lineTopRow    = RowFor(8);
        int lineBottomRow = RowFor(0);
        int perMeasureCols = Mathf.Max(1, (totalCols - (perSystem + 1)) / perSystem);
        int beatsPerMeasure = Mathf.Max(1, data.timeSignature.beatsPerMeasure);

        int cursorCol = 0;
        for (int mi = 0; mi < perSystem; mi++)
        {
            // 맨 왼쪽 시작 마디선은 그리지 않는다 - 마디 사이 구분선만 그린다.
            if (mi > 0)
                DrawBarlineCol(cursorCol, lineTopRow, lineBottomRow);
            cursorCol++;

            int measureIdx = startIdx + mi;
            if (measureIdx < endIdx)
                DrawMeasure(data.measures[measureIdx], cursorCol, perMeasureCols, beatsPerMeasure, RowFor);

            cursorCol += perMeasureCols;
        }

        // 맨 오른쪽 종지 마디선도 그리지 않는다.

        DrawNavTriangles(totalRows, totalCols);

        display.Refresh();
    }

    /// <summary>디스플레이 맨 아래 navRows행에 이전(◀)/다음(▶) 세모를 점자로 찍는다.</summary>
    void DrawNavTriangles(int contentRowCount, int totalCols)
    {
        if (navRows <= 0) return;

        int rowMid = contentRowCount + navRows / 2;
        rowMid = Mathf.Min(rowMid, display.Rows - 1);

        int leftStart  = navSideMargin;
        int leftEnd    = leftStart + navTriangleWidth - 1;
        int rightEnd   = totalCols - 1 - navSideMargin;
        int rightStart = rightEnd - navTriangleWidth + 1;

        DrawTriangle(rowMid, leftStart, leftEnd, pointingLeft: true);   // 이전(◀)
        DrawTriangle(rowMid, rightStart, rightEnd, pointingLeft: false); // 다음(▶)
    }

    /// <summary>
    /// 3행짜리(위/가운데/아래) 채워진 세모.
    /// 가운데 행은 밑변부터 뾰족한 끝까지 전체 폭을 채우고, 위/아래 행은 밑변 쪽 절반만 채워서
    /// 가운데로 갈수록 넓어지는 쐐기(화살촉) 모양이 나오게 한다. (이전엔 위/아래 행이 점 하나뿐이라
    /// 삼각형처럼 안 보였음 - 중간 폭을 둬서 이어지는 모양으로 보이도록 고침)
    /// </summary>
    void DrawTriangle(int rowMid, int cStart, int cEnd, bool pointingLeft)
    {
        int width = cEnd - cStart + 1;
        if (width <= 0) return;

        for (int dr = -1; dr <= 1; dr++)
        {
            int row = rowMid + dr;
            if (row < 0 || row >= display.Rows) continue;

            // 가운데 행(dr=0)은 전체 폭, 위/아래 행(dr=±1)은 절반 폭 - 밑변 쪽에 붙여서
            // 가운데로 갈수록 넓어지는 쐐기 모양을 만든다.
            int widthAtRow = (dr == 0) ? width : Mathf.Max(2, Mathf.RoundToInt(width * 0.5f));

            int start, end;
            if (pointingLeft) { end = cEnd; start = cEnd - widthAtRow + 1; }   // 밑변(넓은 변)이 오른쪽
            else              { start = cStart; end = cStart + widthAtRow - 1; } // 밑변이 왼쪽

            for (int c = start; c <= end; c++)
                display.SetDot(row, c, true);
        }
    }

    void DrawBarlineCol(int col, int rowTop, int rowBottom)
    {
        int top    = Mathf.Min(rowTop, rowBottom);
        int bottom = Mathf.Max(rowTop, rowBottom);
        for (int r = top; r <= bottom; r++)
            display.SetDot(r, col, true);
    }

    private const int StemRows = 5; // 기둥 길이(행 수) - 오선 전체가 16행이라 실제 비율 대신 알아볼 수 있는 정도로 축소

    void DrawMeasure(Measure measure, int startCol, int widthCols, int beatsPerMeasure, System.Func<int, int> rowFor)
    {
        var notes = measure.notes;
        if (notes == null || notes.Count == 0) return;

        // 음표머리는 (막대가 아니라) 원본처럼 박자 위치에 맞춘 한 점으로 찍는다.
        float colsPerBeat = widthCols / (float)beatsPerMeasure;
        var noteCol = new int[notes.Count];
        float cursorBeat = 0f;
        for (int i = 0; i < notes.Count; i++)
        {
            float slotWidth = notes[i].beats * colsPerBeat;
            noteCol[i] = startCol + Mathf.RoundToInt(cursorBeat * colsPerBeat + slotWidth * 0.5f);
            cursorBeat += notes[i].beats;
        }

        // 붙임(beam): 연속된 8분음표 이하는 깃발 대신 기둥 끝을 잇는 가로줄로 표시
        var beamed = new bool[notes.Count];
        for (int i = 0; i < notes.Count - 1; i++)
        {
            if (!beamed[i] && IsEighthOrShorter(notes[i].duration) && IsEighthOrShorter(notes[i + 1].duration))
            {
                beamed[i] = beamed[i + 1] = true;
                DrawBeam(notes[i], noteCol[i], notes[i + 1], noteCol[i + 1], rowFor);
            }
        }

        for (int i = 0; i < notes.Count; i++)
        {
            bool needsFlag = IsEighthOrShorter(notes[i].duration) && !beamed[i];
            DrawNote(notes[i], noteCol[i], rowFor, drawStem: !beamed[i], drawFlag: needsFlag);
        }
    }

    void DrawNote(SheetNote note, int col, System.Func<int, int> rowFor, bool drawStem, bool drawFlag)
    {
        int totalRows = contentRows; // navRows(이전/다음 세모) 영역은 침범하지 않는다
        int totalCols = display.Columns;
        int relStep = RelativeStep(note.pitch);
        int row = rowFor(relStep);
        bool stemUp = relStep < 4; // 가운데 줄(B4) 아래는 기둥 위로, 위는 기둥 아래로 - 원본과 동일
        bool isDotted = note.duration.StartsWith("dotted");

        DrawLedgerLinesIfNeeded(relStep, col, rowFor);
        DrawNoteHead(row, col, totalRows, totalCols);

        int tipRow = row;
        if (drawStem && note.duration != "whole")
        {
            int dir = stemUp ? -1 : 1; // row 0이 맨 위이므로 "위로"는 행 번호 감소
            // 실제 악보처럼: 기둥이 위로 가면 음표머리 오른쪽에, 아래로 가면 왼쪽에 붙인다.
            int stemCol = stemUp ? col + 1 : col - 1;
            stemCol = Mathf.Clamp(stemCol, 0, totalCols - 1);
            tipRow = Mathf.Clamp(row + dir * StemRows, 0, totalRows - 1);
            DrawStemCol(stemCol, row, tipRow);

            if (drawFlag)
            {
                int flagCol = stemUp ? stemCol + 1 : stemCol - 1;
                if (flagCol >= 0 && flagCol < totalCols) display.SetDot(tipRow, flagCol, true);
            }
        }

        if (isDotted)
        {
            int dotCol = col + 2;
            if (dotCol < totalCols) display.SetDot(row, dotCol, true);
        }
    }

    /// <summary>음표머리(동그라미)를 3행×3열 정사각 블록으로 찍어서 확실히 도드라지게 한다.</summary>
    void DrawNoteHead(int row, int col, int totalRows, int totalCols)
    {
        int rTop    = Mathf.Max(0, row - 1);
        int rBottom = Mathf.Min(totalRows - 1, row + 1);
        int cLeft   = Mathf.Max(0, col - 1);
        int cRight  = Mathf.Min(totalCols - 1, col + 1);

        for (int r = rTop; r <= rBottom; r++)
            for (int c = cLeft; c <= cRight; c++)
                display.SetDot(r, c, true);
    }

    void DrawBeam(SheetNote n1, int col1, SheetNote n2, int col2, System.Func<int, int> rowFor)
    {
        int totalRows = contentRows; // navRows(이전/다음 세모) 영역은 침범하지 않는다
        int totalCols = display.Columns;
        int r1 = RelativeStep(n1.pitch);
        int r2 = RelativeStep(n2.pitch);
        bool stemUp = (r1 + r2) < 8; // 두 음의 평균 위치로 기둥 방향을 통일
        int dir = stemUp ? -1 : 1;

        int row1 = rowFor(r1), row2 = rowFor(r2);
        int tip1 = Mathf.Clamp(row1 + dir * StemRows, 0, totalRows - 1);
        int tip2 = Mathf.Clamp(row2 + dir * StemRows, 0, totalRows - 1);

        // 실제 악보처럼: 기둥이 위로 가면 음표머리 오른쪽에, 아래로 가면 왼쪽에 붙인다.
        int stemCol1 = Mathf.Clamp(stemUp ? col1 + 1 : col1 - 1, 0, totalCols - 1);
        int stemCol2 = Mathf.Clamp(stemUp ? col2 + 1 : col2 - 1, 0, totalCols - 1);

        DrawStemCol(stemCol1, row1, tip1);
        DrawStemCol(stemCol2, row2, tip2);

        int beamRow = Mathf.RoundToInt((tip1 + tip2) * 0.5f);
        int cFrom = Mathf.Min(stemCol1, stemCol2), cTo = Mathf.Max(stemCol1, stemCol2);
        for (int c = cFrom; c <= cTo; c++)
            display.SetDot(beamRow, c, true);
    }

    /// <summary>기둥을 1칸 폭(한 줄)으로 그린다.</summary>
    void DrawStemCol(int col, int fromRow, int toRow)
    {
        int r0 = Mathf.Min(fromRow, toRow), r1 = Mathf.Max(fromRow, toRow);
        for (int r = r0; r <= r1; r++)
            display.SetDot(r, col, true);
    }

    void DrawLedgerLinesIfNeeded(int relStep, int col, System.Func<int, int> rowFor)
    {
        if (relStep >= 0 && relStep <= 8) return; // 오선 안이면 덧줄 불필요

        int totalCols = display.Columns;
        int cFrom = Mathf.Max(0, col - 1), cTo = Mathf.Min(totalCols - 1, col + 1);

        if (relStep > 8)
        {
            for (int step = 10; step <= relStep; step += 2)
                for (int c = cFrom; c <= cTo; c++)
                    display.SetDot(rowFor(step), c, true);
        }
        else
        {
            for (int step = -2; step >= relStep; step -= 2)
                for (int c = cFrom; c <= cTo; c++)
                    display.SetDot(rowFor(step), c, true);
        }
    }

    static bool IsEighthOrShorter(string duration) =>
        duration == "eighth" || duration == "dotted-eighth" || duration == "sixteenth";

    static int AbsoluteDiatonic(string pitch)
    {
        char letter = pitch[0];
        int octave  = int.Parse(pitch.Substring(1));
        return octave * 7 + LetterIndex[letter];
    }

    static int RelativeStep(string pitch) => AbsoluteDiatonic(pitch) - AbsoluteDiatonic("E4");
}
