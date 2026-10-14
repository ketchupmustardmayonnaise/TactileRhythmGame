public class ActiveNote
{
    public NoteData data;
    public bool isHit;
    // 예고만 지운 보정 노트도 이후 측정은 가능하다.
    public bool previewDismissed;

    public ActiveNote(NoteData data)
    {
        this.data = data;
    }
}

/// <summary>
/// 판정 결과.
///   None    : 근처에 노트가 없는 헛침(패널티 없음)
///   Miss    : 노트를 놓쳤거나 판정선에서 너무 멀리 침 (perfect/good 외 전부)
///   Good    : good 윈도우 안에서 침
///   Perfect : perfect 윈도우 안에서 침
/// </summary>
public enum HitResult
{
    None,
    Miss,
    Good,
    Perfect
}
