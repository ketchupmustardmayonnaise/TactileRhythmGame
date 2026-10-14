using UnityEngine;

// 모드별 차이는 이 파일에 모은다. 판정·음원·결과 처리는 공통 엔진을 사용한다.
public enum RhythmTestMode
{
    Classic = 0,
    Easy = 1,
    Single = 2,
}

public static class RhythmTestModes
{
    public const RhythmTestMode Default = RhythmTestMode.Classic;

    public static int LaneCount(RhythmTestMode mode) => mode == RhythmTestMode.Single ? 1 : 2;

    public static string ChartSuffix(RhythmTestMode mode) => mode switch
    {
        RhythmTestMode.Easy => "_easy",
        RhythmTestMode.Single => "_single",
        _ => "_2k",
    };

    public static string InputHint(RhythmTestMode mode) =>
        mode == RhythmTestMode.Single ? "Space" : "좌/우 방향키 또는 A / D";

    public static bool IsLaneKeyDown(RhythmTestMode mode, int lane)
    {
        if (lane < 0 || lane >= LaneCount(mode)) return false;
        if (mode == RhythmTestMode.Single) return Input.GetKeyDown(KeyCode.Space);
        return lane == 0
            ? Input.GetKeyDown(KeyCode.LeftArrow) || Input.GetKeyDown(KeyCode.A)
            : Input.GetKeyDown(KeyCode.RightArrow) || Input.GetKeyDown(KeyCode.D);
    }

    public static string OffsetPreferenceKey(RhythmTestMode mode) =>
        mode == RhythmTestMode.Classic ? GameEngine.PrefKeyPreviewOffset
            : GameEngine.PrefKeyPreviewOffset + ChartSuffix(mode);

    public static float CalibrationInterval(RhythmTestMode mode) =>
        mode == RhythmTestMode.Classic ? 0.5f : 1.25f;

    public static float CalibrationPreview(RhythmTestMode mode, float gameplayPreview) =>
        mode == RhythmTestMode.Classic ? 0.5f : gameplayPreview;

    public static bool AllowLegacyChartFallback(RhythmTestMode mode) => mode == RhythmTestMode.Classic;

    public static BrailleCircleButton[] CreateLayout(RhythmTestMode mode,
        BrailleCircleButton[] classicButtons)
    {
        if (mode == RhythmTestMode.Classic) return classicButtons;

        // Classic의 크기와 높이를 유지하고 가로 위치만 변경한다.
        // Easy: 중앙 간격 50% → 24%. 원래 배치는 수정하지 않아 즉시 복귀 가능하다.
        var style = classicButtons[0];
        if (mode == RhythmTestMode.Single)
            return new[] { CopyAt(style, 0.5f) };
        return new[] { CopyAt(style, 0.38f), CopyAt(classicButtons[1], 0.62f) };
    }

    private static BrailleCircleButton CopyAt(BrailleCircleButton source, float column) =>
        new BrailleCircleButton(source.rowRatio, column, source.radiusRatio, source.thicknessRatio);
}
