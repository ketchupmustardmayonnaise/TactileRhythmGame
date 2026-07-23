using UnityEngine;

/// <summary>
/// 효과음(SFX) 재생기. 지금은 키 입력 시 박수 소리를 낸다.
/// 음악용 AudioManager와 별개의 AudioSource를 써서 음악 재생을 방해하지 않는다.
/// </summary>
[RequireComponent(typeof(AudioSource))]
public class SfxPlayer : MonoBehaviour
{
    public static SfxPlayer Instance { get; private set; }

    [Tooltip("키를 눌렀을 때 재생할 박수 소리")]
    public AudioClip clapClip;

    private AudioSource src;

    void Awake()
    {
        if (Instance == null) { Instance = this; }
        else { Destroy(gameObject); return; }

        src = GetComponent<AudioSource>();
        src.playOnAwake = false;
    }

    /// <summary>박수 소리를 한 번 재생.</summary>
    public void PlayClap()
    {
        if (clapClip != null) src.PlayOneShot(clapClip);
    }
}