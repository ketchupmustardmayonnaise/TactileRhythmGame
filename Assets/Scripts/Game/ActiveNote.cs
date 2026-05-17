public class ActiveNote
{
    public NoteData data;
    public float rowPosition;   // 현재 도트 행 위치 (실수)
    public bool isHit;
    public bool isMissed;

    public ActiveNote(NoteData data)
    {
        this.data = data;
    }
}

public enum HitResult
{
    None,
    Good,
    Perfect
}
