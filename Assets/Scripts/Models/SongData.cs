using System;
using System.Collections.Generic;

/// <summary>
/// JSON 채보 파일 형식.
/// {
///   "source": "heavy_serenade.mp3",
///   "difficulty": "easy",
///   "meta": { "bpm": 130.814, "seconds_per_beat": 0.458667, ... },
///   "notes": [ { "time": 0.4267, "lane": 3 }, ... ]
/// }
/// JSON에서 lane은 1-based(1~6). 로드 시 SongLoader가 0-based로 변환한다.
/// </summary>
[Serializable]
public class SongData
{
    /// <summary>오디오 파일명 (예: "heavy_serenade.mp3")</summary>
    public string source;
    public string difficulty;
    public SongMeta meta;
    public List<NoteData> notes;

    /// <summary>source에서 확장자를 뗀 Resources 경로 (예: "Songs/heavy_serenade")</summary>
    public string AudioResourcePath
    {
        get
        {
            if (string.IsNullOrEmpty(source)) return "";
            int dot = source.LastIndexOf('.');
            string name = dot >= 0 ? source.Substring(0, dot) : source;
            return $"Songs/{name}";
        }
    }
}

[Serializable]
public class SongMeta
{
    public float bpm;
    public float seconds_per_beat;
    public float low_high_boundary;
    public float[] low_bounds;
    public float[] high_bounds;
}

[Serializable]
public class NoteData
{
    /// <summary>곡 시작 기준 노트를 쳐야 하는 시각(초)</summary>
    public float time;
    /// <summary>0-based 레인 인덱스 (로드 시 JSON의 1-based에서 변환됨)</summary>
    public int lane;
}
