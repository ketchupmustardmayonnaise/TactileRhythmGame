using UnityEngine;
using UnityEngine.UI;

/// <summary>텍스트 출력 설정과 무관하게 화면 우상단에 표시하는 흰색 삼각형.</summary>
[RequireComponent(typeof(CanvasRenderer))]
public class AutoPlayIndicator : MaskableGraphic
{
    public static AutoPlayIndicator Create(Canvas rootCanvas)
    {
        // 런타임 생성 시에도 Graphic이 활성화되기 전에 렌더러를 확보한다.
        var go = new GameObject("AutoPlayIndicator", typeof(RectTransform), typeof(Canvas), typeof(CanvasRenderer));
        go.transform.SetParent(rootCanvas.transform, false);
        var rect = go.GetComponent<RectTransform>();
        rect.anchorMin = rect.anchorMax = rect.pivot = Vector2.one;
        rect.anchoredPosition = new Vector2(-16f, -16f);
        rect.sizeDelta = new Vector2(44f, 38f);

        // 메뉴/결과 배경보다 앞에 표시. 클릭이나 터치 입력은 가로채지 않는다.
        var canvas = go.GetComponent<Canvas>();
        canvas.overrideSorting = true;
        canvas.sortingOrder = short.MaxValue;
        var indicator = go.AddComponent<AutoPlayIndicator>();
        indicator.color = Color.white;
        indicator.raycastTarget = false;
        go.SetActive(false);
        return indicator;
    }

    protected override void OnPopulateMesh(VertexHelper vh)
    {
        vh.Clear();
        var rect = GetPixelAdjustedRect();
        vh.AddVert(new Vector3(rect.xMin, rect.yMin), color, Vector2.zero);
        vh.AddVert(new Vector3(rect.center.x, rect.yMax), color, Vector2.zero);
        vh.AddVert(new Vector3(rect.xMax, rect.yMin), color, Vector2.zero);
        vh.AddTriangle(0, 1, 2);
    }
}
