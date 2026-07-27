using System;
using System.Collections.Generic;

/// <summary>
/// 오선지(정간보 아님, 5선보) 표시를 위한 악보 데이터.
/// 리듬게임용 SongData(시간/레인 기반)와는 별개로, 음이름/박자/마디 구조를 담는다.
/// </summary>
[Serializable]
public class SheetMusicData
{
    public string title;
    public string composer;

    /// <summary>예: "C Major", "G Major"</summary>
    public string keySignature;
    /// <summary>양수 = #(샤프) 개수, 음수 = b(플랫) 개수, 0 = 조표 없음</summary>
    public int sharpsOrFlats;

    public TimeSignature timeSignature;
    public float tempoBpm;
    /// <summary>빠르기말 (예: "경쾌하게")</summary>
    public string tempoMarking;
    /// <summary>"treble"(높은음자리표)만 지원</summary>
    public string clef;

    public List<Measure> measures;
}

[Serializable]
public class TimeSignature
{
    /// <summary>박자표 위 숫자 (한 마디의 박 수)</summary>
    public int beatsPerMeasure;
    /// <summary>박자표 아래 숫자 (한 박의 기준 음표, 4 = 4분음표)</summary>
    public int beatUnit;
}

[Serializable]
public class Measure
{
    public int number;
    public List<SheetNote> notes;
}

[Serializable]
public class SheetNote
{
    /// <summary>과학적 음이름 (예: "G4", "C5")</summary>
    public string pitch;
    /// <summary>계이름 (예: "솔", "도")</summary>
    public string solfege;
    /// <summary>음표 종류: whole, dotted-half, half, dotted-quarter, quarter, eighth, sixteenth</summary>
    public string duration;
    /// <summary>4분음표 = 1박 기준으로 환산한 박 수</summary>
    public float beats;
}
