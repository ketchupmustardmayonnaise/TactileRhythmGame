using UnityEngine;

[RequireComponent(typeof(AudioSource))]
public class AudioManager : MonoBehaviour
{
    public static AudioManager Instance { get; private set; }

    private AudioSource src;
    // dspTime 기반 정밀 타이밍
    private double scheduledStartDsp;
    private float clipOffset;   // SongData.offset
    private bool scheduled;

    void Awake()
    {
        if (Instance == null) { Instance = this; DontDestroyOnLoad(gameObject); }
        else { Destroy(gameObject); return; }
        EnsureSource();
    }

    /// <summary>
    /// AudioSource(src)가 비어 있으면 다시 확보한다.
    /// Awake 실행 순서나 컴포넌트 누락으로 src가 null이 되어도
    /// SchedulePlay/Stop 등에서 NRE가 나지 않도록 하는 안전장치.
    /// </summary>
    private AudioSource EnsureSource()
    {
        if (src == null)
        {
            src = GetComponent<AudioSource>();
            if (src == null) src = gameObject.AddComponent<AudioSource>();
        }
        return src;
    }

    /// <param name="clip">재생할 클립</param>
    /// <param name="delaySeconds">현재 시점으로부터 실제 재생까지의 대기 시간(초)</param>
    /// <param name="offset">SongData.offset — 오디오 시작 오프셋</param>
    public void SchedulePlay(AudioClip clip, float delaySeconds, float offset = 0f)
    {
        EnsureSource();
        src.clip = clip;
        clipOffset = offset;
        scheduledStartDsp = AudioSettings.dspTime + delaySeconds;
        src.PlayScheduled(scheduledStartDsp);
        scheduled = true;
    }

    public void Stop()
    {
        EnsureSource();
        src.Stop();
        scheduled = false;
    }

    /// <summary>
    /// 게임 타임(초). 재생 전이면 음수 값이 반환된다.
    /// </summary>
    public double SongTime =>
        scheduled ? AudioSettings.dspTime - scheduledStartDsp + clipOffset : 0.0;

    public bool IsPlaying => EnsureSource().isPlaying;
}
