public class ActiveNote
{
    public NoteData data;
    public bool isHit;

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
