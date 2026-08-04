using UnityEngine;

/// <summary>
/// 효과음(SFX) 재생기. 판정 결과별로 다른 소리를 낸다.
///   - Good    → clapClip   (기존 "버튼 누를 때마다 나던" 박수 소리)
///   - Perfect → perfectClip
///   - Miss    → missClip    (타이밍 창 밖에서 빗맞히거나, 안 치고 노트가 지나갈 때)
///
/// 소리는 '키 입력'이 아니라 '판정 확정'(GameEngine.OnJudge)에 맞춰 재생된다.
/// 그래서 노트가 없는 곳을 헛치면(None) 아무 소리도 나지 않고,
/// Good 일 때만 박수가 난다. Miss 소리는 리듬게임 관례대로
/// (1) 빗맞힌 순간과 (2) 노트를 놓쳐 판정 창을 지나가는 순간 모두에 울린다.
///
/// 음악용 AudioManager와 별개의 AudioSource를 써서 음악 재생을 방해하지 않는다.
/// </summary>
[RequireComponent(typeof(AudioSource))]
public class SfxPlayer : MonoBehaviour
{
    public static SfxPlayer Instance { get; private set; }

    [Header("판정 효과음")]
    [Tooltip("Good 판정 때 재생 (기존 박수 소리)")]
    public AudioClip clapClip;
    [Tooltip("Perfect 판정 때 재생")]
    public AudioClip perfectClip;
    [Tooltip("Miss 판정 때 재생 (빗맞히거나 노트를 놓쳐 지나갈 때)")]
    public AudioClip missClip;

    [Header("판정 효과음 볼륨 (1 = 원음, 1보다 크면 더 크게)")]
    [Tooltip("Perfect 소리 배율. Perfect만 더 크게 내고 싶을 때 올린다.")]
    public float perfectVolume = 1.5f;
    [Tooltip("Good(박수) 소리 배율")]
    public float goodVolume = 1f;
    [Tooltip("Miss 소리 배율")]
    public float missVolume = 1f;

    [Tooltip("여러 노트가 같은 순간에 놓쳐도 miss 소리가 겹쳐 울리지 않도록 하는 최소 간격(초).")]
    public float missMinInterval = 0.05f;

    private AudioSource src;
    private float lastMissTime = -999f;

    void Awake()
    {
        if (Instance == null) { Instance = this; }
        else { Destroy(gameObject); return; }

        src = GetComponent<AudioSource>();
        src.playOnAwake = false;
    }

    /// <summary>Good 판정 소리(기존 박수).</summary>
    public void PlayGood() => Play(clapClip, goodVolume);

    /// <summary>Perfect 판정 소리. perfectVolume 만큼 더 크게 낼 수 있다.</summary>
    public void PlayPerfect() => Play(perfectClip, perfectVolume);

    /// <summary>
    /// Miss 판정 소리.
    /// 2key에서 같은 프레임에 두 노트가 동시에 타임아웃될 수 있으므로,
    /// missMinInterval 안에서는 한 번만 울리도록 throttle 한다.
    /// </summary>
    public void PlayMiss()
    {
        float t = Time.unscaledTime;
        if (t - lastMissTime < missMinInterval) return;
        lastMissTime = t;
        Play(missClip, missVolume);
    }

    /// <summary>(하위 호환) 예전 박수 재생 = Good 소리. 타이밍 조정의 탭 피드백 등에서 사용.</summary>
    public void PlayClap() => Play(clapClip, goodVolume);

    // volumeScale 은 PlayOneShot 에서 클립 진폭에 곱해진다. 1보다 크면 더 크게 재생되며,
    // 너무 키우면 소리가 갈라질(클리핑) 수 있으니 적당히. 음수는 0으로 막는다.
    private void Play(AudioClip clip, float volumeScale = 1f)
    {
        if (clip != null && src != null)
            src.PlayOneShot(clip, Mathf.Max(0f, volumeScale));
    }
}