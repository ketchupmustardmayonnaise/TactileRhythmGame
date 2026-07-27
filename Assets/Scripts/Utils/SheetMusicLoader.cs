using UnityEngine;

public static class SheetMusicLoader
{
    /// <summary>
    /// Resources/Songs/ 안의 5선보 JSON 파일을 로드한다.
    /// </summary>
    /// <param name="resourceName">파일명 (확장자 제외, 예: "santoki_sheet")</param>
    public static SheetMusicData LoadFromResources(string resourceName)
    {
        TextAsset asset = Resources.Load<TextAsset>($"Songs/{resourceName}");
        if (asset == null)
        {
            Debug.LogError($"[SheetMusicLoader] 찾을 수 없음: Resources/Songs/{resourceName}.json");
            return null;
        }
        return JsonUtility.FromJson<SheetMusicData>(asset.text);
    }
}
