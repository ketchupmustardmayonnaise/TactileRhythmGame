using System.Collections.Generic;
using System.Text.RegularExpressions;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

/// <summary>텍스트 내용과 노트 시각화를 분리한다. 콘솔은 변경된 내용만 출력한다.</summary>
public sealed class GameTextOutput
{
    private readonly Dictionary<string, string> published = new();
    private readonly Dictionary<Graphic, bool> visibility = new();
    public bool ConsoleOnly { get; private set; }

    public void SetMode(bool consoleOnly, MonoBehaviour owner, IEnumerable<Graphic> overlays)
    {
        RestoreVisibility();
        published.Clear();
        ConsoleOnly = consoleOnly;
        if (!consoleOnly) return;

        // 비활성 메뉴·결과의 텍스트도 수집하여 이후 활성화될 때 글자가 새지 않게 한다.
        foreach (var text in Object.FindObjectsOfType<TMP_Text>(true))
            if (text.gameObject.scene == owner.gameObject.scene) Hide(text);
        foreach (var text in Object.FindObjectsOfType<Text>(true))
            if (text.gameObject.scene == owner.gameObject.scene) Hide(text);
        foreach (var graphic in overlays) Hide(graphic);
    }

    private void Hide(Graphic graphic)
    {
        if (graphic == null || visibility.ContainsKey(graphic)) return;
        visibility[graphic] = graphic.enabled;
        graphic.enabled = false;
    }

    public void RestoreVisibility()
    {
        foreach (var entry in visibility)
            if (entry.Key != null) entry.Key.enabled = entry.Value;
        visibility.Clear();
    }

    public void Write(string channel, TMP_Text target, string message, bool force = false)
    {
        if (target != null && target.text != message) target.text = message;
        if (string.IsNullOrEmpty(message)) { published.Remove(channel); return; }
        if (!ConsoleOnly) return;
        if (!force && published.TryGetValue(channel, out string previous) && previous == message) return;
        published[channel] = message;
        Debug.Log($"[{channel}] {Regex.Replace(message, @"<[^>]+>", "")}");
    }

    public void PublishOtherText(MonoBehaviour owner, HashSet<TMP_Text> managed)
    {
        if (!ConsoleOnly) return;
        foreach (var text in Object.FindObjectsOfType<TMP_Text>(true))
            if (text.gameObject.scene == owner.gameObject.scene && text.gameObject.activeInHierarchy
                && !managed.Contains(text)) Write("UI:" + text.GetInstanceID(), text, text.text);
        foreach (var text in Object.FindObjectsOfType<Text>(true))
            if (text.gameObject.scene == owner.gameObject.scene && text.gameObject.activeInHierarchy)
                Write("UI:" + text.GetInstanceID(), null, text.text);
    }
}
