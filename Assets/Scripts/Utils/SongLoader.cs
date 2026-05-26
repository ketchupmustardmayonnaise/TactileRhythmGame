using UnityEngine;

/// <summary>
/// Resources/Songs/ 폴더의 JSON 채보 파일을 로드한다.
/// JSON의 lane(1-based)을 0-based로 자동 변환한다.
/// </summary>
public static class SongLoader
{
    /// <param name="resourceName">파일명 (확장자 제외, 예: "heavy_serenade")</param>
    public static SongData LoadFromResources(string resourceName)
    {
        TextAsset asset = Resources.Load<TextAsset>($"Songs/{resourceName}");
        if (asset == null)
        {
            Debug.LogError($"[SongLoader] 찾을 수 없음: Resources/Songs/{resourceName}.json");
            return null;
        }
        return Parse(asset.text);
    }

    /// <summary>절대 경로에서 로드. 유저 커스텀 채보용.</summary>
    public static SongData LoadFromPath(string fullPath)
    {
        if (!System.IO.File.Exists(fullPath))
        {
            Debug.LogError($"[SongLoader] 파일 없음: {fullPath}");
            return null;
        }
        return Parse(System.IO.File.ReadAllText(fullPath));
    }

    static SongData Parse(string json)
    {
        SongData song = JsonUtility.FromJson<SongData>(json);

        // JSON lane은 1-based(1~6) → 내부에서는 0-based(0~5)로 사용
        if (song?.notes != null)
            foreach (var n in song.notes)
                n.lane -= 1;

        return song;
    }
}
