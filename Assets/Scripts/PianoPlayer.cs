using System.Collections;
using System.Collections.Generic;
using UnityEngine;

/// <summary>
/// 계이름(예: "C4", "D#4", "A4")을 받아 피아노 비슷한 음을 재생합니다.
/// 오디오 파일이 필요 없습니다 — 주파수를 계산해 하모닉을 섞고 감쇠 엔벨로프를
/// 입힌 AudioClip을 코드로 생성합니다.
///
/// 사용법:
///   1. 빈 GameObject에 이 스크립트를 붙입니다. (AudioSource 자동 추가)
///   2. 다른 스크립트에서:
///        pianoPlayer.PlayNote("C4");
///        pianoPlayer.PlayNote("E4");
///        pianoPlayer.PlayNote("G4");
///      또는 한국식 계이름도 지원합니다:
///        pianoPlayer.PlayNote("도");   // 기본 옥타브(4) 사용
///        pianoPlayer.PlayNote("레5");  // 옥타브 지정
///
/// 표기법:
///   - 영어식: C, D, E, F, G, A, B + (#/b 선택) + 옥타브 숫자  예) "C4", "F#5", "Bb3"
///   - 한국식: 도 레 미 파 솔 라 시 + (옥타브 숫자 선택)        예) "도", "솔4", "라5"
/// </summary>
[RequireComponent(typeof(AudioSource))]
public class PianoPlayer : MonoBehaviour
{
    [Header("Tone")]
    [Tooltip("한 음의 길이 (초)")]
    public float noteDuration = 0.8f;

    [Range(0f, 1f)]
    public float volume = 0.6f;

    [Tooltip("표기에 옥타ब이 없을 때 사용할 기본 옥타브")]
    public int defaultOctave = 4;

    private AudioSource _audioSource;
    private const int SampleRate = 44100;

    // 한 옥타브 안에서 C를 0으로 한 반음 인덱스
    private static readonly Dictionary<string, int> NoteToSemitone = new Dictionary<string, int>
    {
        { "C", 0 }, { "D", 2 }, { "E", 4 }, { "F", 5 },
        { "G", 7 }, { "A", 9 }, { "B", 11 },
    };

    // 한국식 계이름 → 영어식 음이름
    private static readonly Dictionary<string, string> KoreanToNote = new Dictionary<string, string>
    {
        { "도", "C" }, { "레", "D" }, { "미", "E" }, { "파", "F" },
        { "솔", "G" }, { "라", "A" }, { "시", "B" },
    };

    private void Awake()
    {
        _audioSource = GetComponent<AudioSource>();
        _audioSource.playOnAwake = false;
    }

    /// <summary>계이름 문자열을 받아 음을 재생합니다. 인식 실패 시 경고 후 무시.</summary>
    public void PlayNote(string noteName)
    {
        if (!TryParseNote(noteName, out float frequency))
        {
            Debug.LogWarning($"[PianoPlayer] 계이름을 해석하지 못했습니다: '{noteName}'");
            return;
        }
        AudioClip clip = CreatePianoClip(frequency, noteName);
        _audioSource.PlayOneShot(clip, volume);
    }

    /// <summary>주파수(Hz)를 직접 지정해 재생합니다.</summary>
    public void PlayFrequency(float frequency)
    {
        AudioClip clip = CreatePianoClip(frequency, $"{frequency:F1}Hz");
        _audioSource.PlayOneShot(clip, volume);
    }

    /// <summary>계이름을 주파수(Hz)로 변환합니다. A4 = 440Hz 기준 12평균율.</summary>
    public bool TryParseNote(string raw, out float frequency)
    {
        frequency = 0f;
        if (string.IsNullOrWhiteSpace(raw)) return false;

        string input = raw.Trim();
        string letter;
        int accidental = 0; // #: +1, b: -1
        int index = 0;

        // 1) 한국식 계이름 우선 처리
        string firstChar = input.Substring(0, 1);
        if (KoreanToNote.ContainsKey(firstChar))
        {
            letter = KoreanToNote[firstChar];
            index = 1;
        }
        else
        {
            // 2) 영어식: 첫 글자는 음이름(대소문자 허용)
            letter = firstChar.ToUpperInvariant();
            if (!NoteToSemitone.ContainsKey(letter)) return false;
            index = 1;

            // 임시표(#, b) 처리
            if (index < input.Length)
            {
                char c = input[index];
                if (c == '#') { accidental = 1; index++; }
                else if (c == 'b' || c == 'B') { accidental = -1; index++; }
            }
        }

        // 3) 남은 부분이 옥타브 숫자(없으면 기본 옥타브)
        int octave = defaultOctave;
        if (index < input.Length)
        {
            string octaveStr = input.Substring(index);
            if (!int.TryParse(octaveStr, out octave)) return false;
        }

        // 4) MIDI 음높이 → 주파수
        // MIDI noteNumber = (octave + 1) * 12 + semitone   (C-1 = 0, A4 = 69)
        int semitone = NoteToSemitone[letter] + accidental;
        int midiNote = (octave + 1) * 12 + semitone;
        frequency = 440f * Mathf.Pow(2f, (midiNote - 69) / 12f);
        return true;
    }

    /// <summary>
    /// 피아노 비슷한 음색의 AudioClip을 생성합니다.
    /// 기음 + 약한 배음 몇 개를 더하고, 빠른 어택 + 느린 감쇠 엔벨로프를 적용합니다.
    /// </summary>
    private AudioClip CreatePianoClip(float frequency, string clipName)
    {
        int sampleCount = Mathf.CeilToInt(SampleRate * noteDuration);
        float[] samples = new float[sampleCount];

        // 배음 구성 (기음 1.0, 2배음 0.5 ...) — 피아노 느낌을 위한 단순 가산합성
        float[] harmonicGains = { 1.0f, 0.5f, 0.25f, 0.12f, 0.06f };

        for (int i = 0; i < sampleCount; i++)
        {
            float t = (float)i / SampleRate;

            float value = 0f;
            for (int h = 0; h < harmonicGains.Length; h++)
            {
                float harmonicFreq = frequency * (h + 1);
                value += harmonicGains[h] * Mathf.Sin(2f * Mathf.PI * harmonicFreq * t);
            }

            // 엔벨로프: 5ms 어택, 이후 지수 감쇠
            float attackTime = 0.005f;
            float envelope;
            if (t < attackTime)
                envelope = t / attackTime;
            else
                envelope = Mathf.Exp(-(t - attackTime) * 4f);

            samples[i] = value * envelope * 0.3f; // 0.3: 배음 합산 클리핑 방지
        }

        AudioClip clip = AudioClip.Create(clipName, sampleCount, 1, SampleRate, false);
        clip.SetData(samples, 0);
        return clip;
    }
}
