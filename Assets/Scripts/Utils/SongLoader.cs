using UnityEngine;

public static class SongLoader
{
    /// <summary>
    /// Resources/Songs/ 폴더 안의 JSON 파일을 로드한다.
    /// </summary>
    /// <param name="resourceName">파일명 (확장자 제외, 예: "sample")</param>
    public static SongData LoadFromResources(string resourceName)
    {
        TextAsset asset = Resources.Load<TextAsset>($"Songs/{resourceName}");
        if (asset == null)
        {
            Debug.LogError($"[SongLoader] 찾을 수 없음: Resources/Songs/{resourceName}.json");
            return null;
        }
        return JsonUtility.FromJson<SongData>(asset.text);
    }

    /// <summary>
    /// 절대 경로 또는 Application.persistentDataPath 기반 경로에서 로드.
    /// 유저가 커스텀 채보를 넣을 수 있는 경로용.
    /// </summary>
    public static SongData LoadFromPath(string fullPath)
    {
        if (!System.IO.File.Exists(fullPath))
        {
            Debug.LogError($"[SongLoader] 파일 없음: {fullPath}");
            return null;
        }
        string json = System.IO.File.ReadAllText(fullPath);
        return JsonUtility.FromJson<SongData>(json);
    }
}
