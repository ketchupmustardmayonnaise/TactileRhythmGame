using System;
using System.Collections;
using System.Collections.Generic;
using UnityEngine;

/// <summary>
/// dspTime ����� ��Ȯ�� ��Ʈ�γ�.
/// BPM�� �ְ� StartMetronome()�� ȣ���ϸ� StopMetronome()�� �θ� ������ ���ڸ� ���ϴ�.
/// Ŭ�� ����� �ڵ�� �����ϹǷ� ���� ����� ������ �ʿ� �����ϴ�.
///
/// ����:
///   1. �� GameObject�� �� ��ũ��Ʈ�� ���Դϴ�. (AudioSource�� �ڵ� �߰�)
///   2. �ν����Ϳ��� bpm�� �����ϰų�, �ٸ� ��ũ��Ʈ���� StartMetronome(140f) ȣ��.
///   3. ���߷��� StopMetronome().
///   4. �� �ڸ��� ���� �ϰ� ������ OnBeat �̺�Ʈ�� �����ϼ���.
/// </summary>

public class Metronome : MonoBehaviour
{
    [Header("Tempo")]
    [Tooltip("�д� �� �� (Beats Per Minute)")]
    [Range(20f, 300f)]
    public float bpm = 120f;

    [Tooltip("�� ������ �� ��. ù ���� �������� �����մϴ�. (��: 4 = 4/4����)")]
    [Range(1, 16)]
    public int beatsPerBar = 4;

    [Header("Click Sound")]
    [Tooltip("����(���� ù ��) ���ļ� (Hz)")]
    public float accentFrequency = 1600f;

    [Tooltip("��� ���ļ� (Hz)")]
    public float normalFrequency = 1000f;

    [Tooltip("Ŭ�� ���� (��)")]
    public float clickDuration = 0.05f;

    [Range(0f, 1f)]
    public float volume = 0.5f;

    /// <summary>�� �ڸ��� ȣ��˴ϴ�. ���ڴ� ���� �� �� ��ȣ(0���� ����, 0�� ����).</summary>
    public event Action<int> OnBeat;

    public bool IsRunning { get; private set; }

    private AudioSource _audioSource;
    private AudioClip _accentClip;
    private AudioClip _normalClip;

    private double _nextBeatDspTime;   // ���� ���� ����� �� dsp �ð�
    private int _beatInBar;            // ���� ���� �� �� �ε���
    private const int SampleRate = 44100;

    private void Awake()
    {
        _audioSource = GetComponent<AudioSource>();
        _audioSource.playOnAwake = false;

        _accentClip = CreateClickClip(accentFrequency, "MetronomeAccent");
        _normalClip = CreateClickClip(normalFrequency, "MetronomeNormal");

        // 자동 시작하지 않는다. StartMetronome() 호출 시에만 동작.
        // (예전에는 여기서 IsRunning=true 였으나 _nextBeatDspTime 미초기화로
        //  Update의 while 루프가 폭주해 오디오 채널을 고갈시키는 버그가 있었음)
        IsRunning = false;
    }

    /// <summary>���� bpm���� ��Ʈ�γ��� �����մϴ�.</summary>
    public void StartMetronome()
    {
        if (IsRunning) return;
        IsRunning = true;
        _beatInBar = 0;
        // ��¦ ����(0.1��)�� �ΰ� ù �� ���� �� ù ���� ������ �ʰ� ��
        _nextBeatDspTime = AudioSettings.dspTime + 0.1;
    }

    /// <summary>bpm�� �ٲٸ鼭 �����ϴ� ���� �޼���.</summary>
    public void StartMetronome(float newBpm)
    {
        bpm = Mathf.Clamp(newBpm, 20f, 300f);
        StartMetronome();
    }

    /// <summary>��Ʈ�γ��� ����ϴ�.</summary>
    public void StopMetronome()
    {
        IsRunning = false;
    }

    /// <summary>���� �߿� BPM�� �ٲ㵵 ���� �ں��� ��� �ݿ��˴ϴ�.</summary>
    public void SetBpm(float newBpm)
    {
        bpm = Mathf.Clamp(newBpm, 20f, 300f);
    }

    private void Update()
    {
        if (!IsRunning) return;

        double secondsPerBeat = 60.0 / bpm;

        // dspTime�� ���� ���� �������� ���� �︲.
        // while���̶� ū ������ ����� ���� �и� ���� ��� ��������.
        while (AudioSettings.dspTime >= _nextBeatDspTime)
        {
            bool isAccent = (_beatInBar == 0);
            _audioSource.PlayOneShot(isAccent ? _accentClip : _normalClip, volume);

            OnBeat?.Invoke(_beatInBar);

            _beatInBar = (_beatInBar + 1) % Mathf.Max(1, beatsPerBar);
            _nextBeatDspTime += secondsPerBeat;
        }
    }

    /// <summary>������ ���ļ��� ª�� ���� Ŭ���� AudioClip�� �ڵ� �����մϴ�.</summary>
    private AudioClip CreateClickClip(float frequency, string clipName)
    {
        int sampleCount = Mathf.CeilToInt(SampleRate * clickDuration);
        float[] samples = new float[sampleCount];

        for (int i = 0; i < sampleCount; i++)
        {
            float t = (float)i / SampleRate;
            // ������ �����ϴ� �������� �� "ƽ" �ϴ� ª�� �Ҹ�
            float envelope = Mathf.Exp(-t * 40f);
            samples[i] = Mathf.Sin(2f * Mathf.PI * frequency * t) * envelope;
        }

        AudioClip clip = AudioClip.Create(clipName, sampleCount, 1, SampleRate, false);
        clip.SetData(samples, 0);
        return clip;
    }
}
