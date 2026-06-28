using System.Collections.Generic;
using UnityEngine;

/// <summary>
/// 점자(6점) 텍스트를 BrailleCellDisplay 위에 점 단위로 렌더링한다.
/// 디스플레이 상단 영역에 음악 계이름·박자 등을 점자로 띄우는 용도.
///
/// 점 번호 배치 (표준 6점 점자):
///     1 4
///     2 5
///     3 6
/// 한 글자는 (dotGap+1) 칸 너비 × (2*dotGap+1) 칸 높이를 차지하고,
/// 글자 사이에 charGap 칸을 둔다.
/// </summary>
public static class BrailleText
{
    // 점 번호(1~6) → 셀 그리드의 (행오프셋, 열오프셋). dotGap 곱은 Render에서 적용.
    static readonly (int row, int col)[] DotOffset =
    {
        (0, 0), // 1
        (1, 0), // 2
        (2, 0), // 3
        (0, 1), // 4
        (1, 1), // 5
        (2, 1), // 6
    };

    // 문자 → 활성화할 점 번호들 (표준 영어 점자 grade-1)
    static readonly Dictionary<char, int[]> Map = new()
    {
        ['a'] = new[]{1},        ['b'] = new[]{1,2},      ['c'] = new[]{1,4},
        ['d'] = new[]{1,4,5},    ['e'] = new[]{1,5},      ['f'] = new[]{1,2,4},
        ['g'] = new[]{1,2,4,5},  ['h'] = new[]{1,2,5},    ['i'] = new[]{2,4},
        ['j'] = new[]{2,4,5},    ['k'] = new[]{1,3},      ['l'] = new[]{1,2,3},
        ['m'] = new[]{1,3,4},    ['n'] = new[]{1,3,4,5},  ['o'] = new[]{1,3,5},
        ['p'] = new[]{1,2,3,4},  ['q'] = new[]{1,2,3,4,5},['r'] = new[]{1,2,3,5},
        ['s'] = new[]{2,3,4},    ['t'] = new[]{2,3,4,5},  ['u'] = new[]{1,3,6},
        ['v'] = new[]{1,2,3,6},  ['w'] = new[]{2,4,5,6},  ['x'] = new[]{1,3,4,6},
        ['y'] = new[]{1,3,4,5,6},['z'] = new[]{1,3,5,6},
        ['#'] = new[]{3,4,5,6},  // 숫자 기호
    };

    // 숫자 0~9 → a~j 와 동일한 점 패턴 (앞에 숫자 기호 '#' 를 붙여 표기)
    static readonly char[] DigitAsLetter =
        { 'j', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i' };

    /// <summary>
    /// text를 점자로 렌더링한다.
    /// </summary>
    /// <param name="startRow">첫 글자 점1의 행</param>
    /// <param name="startCol">첫 글자 점1의 열</param>
    /// <param name="dotGap">한 글자 안에서 점 사이 간격(칸)</param>
    /// <param name="charGap">글자 사이 간격(칸)</param>
    /// <param name="activation">점 밝기(0~1)</param>
    public static void Render(BrailleCellDisplay display, string text,
                              int startRow, int startCol,
                              int dotGap = 2, int charGap = 3, float activation = 1f)
    {
        if (display == null || string.IsNullOrEmpty(text)) return;

        int col       = startCol;
        int charWidth = dotGap + 1;            // 점 2열이 dotGap 떨어져 있으므로
        int advance   = charWidth + charGap;

        foreach (char raw in text.ToLowerInvariant())
        {
            if (raw == ' ') { col += advance; continue; }

            if (raw >= '0' && raw <= '9')
            {
                // 숫자 기호 + 대응 문자
                DrawCell(display, '#', startRow, col, dotGap, activation);
                col += advance;
                DrawCell(display, DigitAsLetter[raw - '0'], startRow, col, dotGap, activation);
                col += advance;
                continue;
            }

            DrawCell(display, raw, startRow, col, dotGap, activation);
            col += advance;
        }
    }

    /// <summary>가로 중앙 정렬해서 렌더링.</summary>
    public static void RenderCentered(BrailleCellDisplay display, string text,
                                      int startRow, int dotGap = 2, int charGap = 3,
                                      float activation = 1f)
    {
        if (display == null || string.IsNullOrEmpty(text)) return;
        int width    = Measure(text, dotGap, charGap);
        int startCol = Mathf.Max((display.Columns - width) / 2, 0);
        Render(display, text, startRow, startCol, dotGap, charGap, activation);
    }

    /// <summary>렌더링 시 차지하는 가로 칸 수.</summary>
    public static int Measure(string text, int dotGap = 2, int charGap = 3)
    {
        if (string.IsNullOrEmpty(text)) return 0;
        int advance = (dotGap + 1) + charGap;
        int count   = 0;
        foreach (char raw in text)
            count += (raw >= '0' && raw <= '9') ? 2 : 1;   // 숫자는 기호+문자 = 2칸
        return count * advance - charGap;
    }

    static void DrawCell(BrailleCellDisplay display, char ch,
                         int startRow, int startCol, int dotGap, float activation)
    {
        if (!Map.TryGetValue(ch, out int[] dots)) return;
        foreach (int dot in dots)
        {
            var off = DotOffset[dot - 1];
            display.SetDotActivation(startRow + off.row * dotGap,
                                     startCol + off.col * dotGap,
                                     activation);
        }
    }
}
