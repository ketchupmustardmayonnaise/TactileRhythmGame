using System;
using System.Collections.Generic;

[Serializable]
public class SongData
{
    public string id;
    public string title;
    public string artist;
    public float bpm;
    /// <summary>Resources 폴더 기준 오디오 파일 경로 (확장자 제외)</summary>
    public string audioFile;
    /// <summary>오디오 시작 오프셋(초). 양수면 음악이 늦게 시작, 음수면 일찍 시작</summary>
    public float offset;
    public List<NoteData> notes;
}

[Serializable]
public class NoteData
{
    /// <summary>곡 시작 기준 노트를 쳐야 하는 시각(초)</summary>
    public float time;
    /// <summary>0부터 시작하는 레인 인덱스</summary>
    public int lane;
    /// <summary>"tap" 또는 "hold"</summary>
    public string type;
    /// <summary>hold 타입일 때 길이(초)</summary>
    public float duration;
}
