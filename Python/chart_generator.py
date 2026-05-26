"""
6키 리듬게임 자동 채보 생성기 v2
──────────────────────────────────────────────────────────
v1 대비 개선:
  1. 저/고음 분류: centroid(고주파 편향) → 에너지 비율 방식
  2. 저음/고음 경계 및 각 구간 3등분 경계: 노래 분석으로 자동 결정
  3. BPM (tempo) 감지 및 채보 헤더 저장

사용법:
    python chart_generator_v2.py <오디오파일> [--difficulty easy|medium|hard] [--output out.chart]
"""

import sys, argparse
import numpy as np

try:
    import librosa
except ImportError:
    sys.exit("librosa가 없습니다. pip install librosa 로 설치해 주세요.")


# ── 난이도 설정 ────────────────────────────────────────────
DIFFICULTY = {
    "easy":   {"delta": 0.12, "wait": 0.15},
    "medium": {"delta": 0.07, "wait": 0.10},
    "hard":   {"delta": 0.03, "wait": 0.05},
}


# ═══════════════════════════════════════════════════════════
# 1. 주파수 경계 자동 분석
# ═══════════════════════════════════════════════════════════

def analyze_boundaries(y, sr, onset_frames, hop_length=512, n_fft=2048):
    """
    노래를 분석해서 다음 경계값을 자동으로 계산합니다.

    방법:
      - 각 onset 프레임의 스펙트럼에서 '저음 에너지 중심 주파수'와
        '고음 에너지 중심 주파수'를 구함
      - 저/고음 분리 경계: 전체 평균 스펙트럼의 누적에너지 50% 지점 (spectral median)
      - 각 구간 3등분: onset별 주파수를 모아 33/66 퍼센타일

    Returns:
        low_high_boundary (Hz),
        low_bounds [f0, f1, f2, f3],
        high_bounds [f0, f1, f2, f3]
    """
    S = np.abs(librosa.stft(y, n_fft=n_fft, hop_length=hop_length))
    freqs = librosa.fft_frequencies(sr=sr, n_fft=n_fft)

    # ── 저/고음 경계: 전체 평균 스펙트럼의 spectral median ──
    mean_spectrum = S.mean(axis=1)
    cum_energy    = np.cumsum(mean_spectrum)
    half_energy   = cum_energy[-1] * 0.50
    boundary_idx  = np.searchsorted(cum_energy, half_energy)
    low_high_boundary = float(freqs[boundary_idx])

    # 너무 낮거나 높으면 클리핑 (50~800 Hz 범위로 제한)
    low_high_boundary = float(np.clip(low_high_boundary, 50.0, 800.0))

    print(f"  ↳ 저/고음 자동 경계: {low_high_boundary:.1f} Hz")

    # ── onset별 저음/고음 대표 주파수 수집 ──────────────────
    low_freqs_list  = []
    high_freqs_list = []

    for frame in onset_frames:
        frame = min(int(frame), S.shape[1] - 1)
        spectrum = S[:, frame]

        low_mask  = freqs <  low_high_boundary
        high_mask = freqs >= low_high_boundary

        low_energy  = spectrum[low_mask]
        high_energy = spectrum[high_mask]

        # 저음 대표 주파수 (저음 구간 내 centroid)
        total_low = low_energy.sum()
        if total_low > 1e-6:
            low_centroid = float(np.sum(freqs[low_mask] * low_energy) / total_low)
            low_freqs_list.append(low_centroid)

        # 고음 대표 주파수 (고음 구간 내 centroid)
        total_high = high_energy.sum()
        if total_high > 1e-6:
            high_centroid = float(np.sum(freqs[high_mask] * high_energy) / total_high)
            high_freqs_list.append(high_centroid)

    # ── 저음 3등분 경계: 0 / 33% / 66% / max percentile ────
    if len(low_freqs_list) >= 3:
        p33_low  = float(np.percentile(low_freqs_list, 33))
        p66_low  = float(np.percentile(low_freqs_list, 66))
    else:
        p33_low  = low_high_boundary * 0.33
        p66_low  = low_high_boundary * 0.66

    # ── 고음 3등분 경계: min / 33% / 66% / 20000 ────────────
    if len(high_freqs_list) >= 3:
        p33_high = float(np.percentile(high_freqs_list, 33))
        p66_high = float(np.percentile(high_freqs_list, 66))
    else:
        p33_high = low_high_boundary + (sr / 2 - low_high_boundary) * 0.33
        p66_high = low_high_boundary + (sr / 2 - low_high_boundary) * 0.66

    low_bounds  = [0.0,               p33_low,  p66_low,  low_high_boundary]
    high_bounds = [low_high_boundary, p33_high, p66_high, sr / 2]

    print(f"  ↳ 저음 3등분 경계: {low_bounds[1]:.1f} / {low_bounds[2]:.1f} Hz")
    print(f"  ↳ 고음 3등분 경계: {high_bounds[1]:.1f} / {high_bounds[2]:.1f} Hz")

    return low_high_boundary, low_bounds, high_bounds


# ═══════════════════════════════════════════════════════════
# 2. 레인 결정
# ═══════════════════════════════════════════════════════════

def get_lane(spectrum, freqs, low_high_boundary, low_bounds, high_bounds, prev_lane):
    """
    에너지 비율로 저/고음 그룹 결정 → 해당 그룹 내 서브밴드 에너지로 레인 결정.
    """
    low_mask  = freqs <  low_high_boundary
    high_mask = freqs >= low_high_boundary

    low_energy  = spectrum[low_mask].sum()
    high_energy = spectrum[high_mask].sum()

    # ── 그룹 결정 ─────────────────────────────────────────
    is_low = low_energy >= high_energy  # 저음 에너지가 크면 저음 그룹

    if is_low:
        bounds     = low_bounds
        base_lane  = 1
        group_mask = low_mask
    else:
        bounds     = high_bounds
        base_lane  = 4
        group_mask = high_mask

    # ── 서브밴드 중 에너지 최대 구간 → 레인 ──────────────
    best_lane   = base_lane
    best_energy = -1.0

    for i in range(3):
        sub_mask = (freqs >= bounds[i]) & (freqs < bounds[i + 1])
        sub_e    = spectrum[sub_mask].sum()
        if sub_e > best_energy:
            best_energy = sub_e
            best_lane   = base_lane + i

    # ── 같은 레인 연속 방지 ───────────────────────────────
    if best_lane == prev_lane:
        group_start = 1 if best_lane <= 3 else 4
        group_end   = 3 if best_lane <= 3 else 6
        candidates  = [l for l in range(group_start, group_end + 1) if l != best_lane]
        best_lane   = min(candidates, key=lambda l: abs(l - best_lane))

    return best_lane


# ═══════════════════════════════════════════════════════════
# 3. BPM 분석
# ═══════════════════════════════════════════════════════════

def analyze_bpm(y, sr):
    """
    tempo (BPM) 와 beat 타이밍을 반환.
    beat_times: 각 박자가 시작되는 초 단위 리스트
    """
    tempo, beat_frames = librosa.beat.beat_track(y=y, sr=sr)
    beat_times = librosa.frames_to_time(beat_frames, sr=sr).tolist()
    bpm = float(tempo) if np.isscalar(tempo) else float(tempo[0])
    spb = 60.0 / bpm   # seconds per beat
    print(f"  ↳ BPM: {bpm:.2f}  (한 박자 = {spb:.4f}초)")
    return bpm, spb, beat_times


# ═══════════════════════════════════════════════════════════
# 4. 채보 생성 메인
# ═══════════════════════════════════════════════════════════

def generate_chart(audio_path: str, difficulty: str = "medium"):
    print(f"[1/5] 오디오 로딩: {audio_path}")
    y, sr = librosa.load(audio_path, sr=None, mono=True)

    print("[2/5] BPM 분석...")
    bpm, spb, beat_times = analyze_bpm(y, sr)

    print("[3/5] Onset 감지...")
    cfg = DIFFICULTY[difficulty]
    hop = 512
    onset_frames = librosa.onset.onset_detect(
        y=y, sr=sr,
        delta=cfg["delta"],
        wait=int(cfg["wait"] * sr / hop),
        backtrack=True,
    )
    onset_times = librosa.frames_to_time(onset_frames, sr=sr)
    print(f"  ↳ onset {len(onset_times)}개 감지")

    print("[4/5] 주파수 경계 자동 분석...")
    n_fft = 2048
    low_high_boundary, low_bounds, high_bounds = analyze_boundaries(
        y, sr, onset_frames, hop_length=hop, n_fft=n_fft
    )

    print("[5/5] 레인 배치...")
    S     = np.abs(librosa.stft(y, n_fft=n_fft, hop_length=hop))
    freqs = librosa.fft_frequencies(sr=sr, n_fft=n_fft)

    notes     = []
    prev_lane = -1

    for t in onset_times:
        frame = librosa.time_to_frames(t, sr=sr, hop_length=hop)
        frame = min(int(frame), S.shape[1] - 1)
        spectrum = S[:, frame]

        if spectrum.sum() < 1e-6:
            continue

        lane = get_lane(spectrum, freqs, low_high_boundary, low_bounds, high_bounds, prev_lane)
        notes.append((round(float(t), 4), lane))
        prev_lane = lane

    low_count  = sum(1 for _, l in notes if l <= 3)
    high_count = sum(1 for _, l in notes if l >= 4)
    print(f"  ↳ 완료: 총 {len(notes)}노트  (저음 {low_count} / 고음 {high_count})")

    meta = {
        "bpm": bpm,
        "seconds_per_beat": spb,
        "low_high_boundary": low_high_boundary,
        "low_bounds": low_bounds,
        "high_bounds": high_bounds,
    }
    return notes, meta


# ═══════════════════════════════════════════════════════════
# 5. 저장 & 미리보기
# ═══════════════════════════════════════════════════════════

def save_chart(notes, meta, output_path, audio_path, difficulty):
    import json

    data = {
        "source":     audio_path,
        "difficulty": difficulty,
        "meta": {
            "bpm":               round(meta["bpm"], 4),
            "seconds_per_beat":  round(meta["seconds_per_beat"], 6),
            "low_high_boundary": round(meta["low_high_boundary"], 2),
            "low_bounds":        [round(v, 2) for v in meta["low_bounds"]],
            "high_bounds":       [round(v, 2) for v in meta["high_bounds"]],
        },
        "notes": [
            {"time": t, "lane": lane}
            for t, lane in notes
        ],
    }

    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)

    print(f"\n채보 저장 완료: {output_path}  ({len(notes)}노트)")


def print_preview(notes, max_rows=40):
    print("\n── 채보 미리보기 ─────────────────────────────────")
    print("  시간(초)   레인   1  2  3 | 4  5  6")
    print("  " + "-" * 40)
    for t, lane in notes[:max_rows]:
        cols = ["·"] * 6
        cols[lane - 1] = "■"
        low_s  = "  ".join(cols[:3])
        high_s = "  ".join(cols[3:])
        group  = "저음" if lane <= 3 else "고음"
        print(f"  {t:7.3f}s  [{lane}]   {low_s} | {high_s}   {group}")
    if len(notes) > max_rows:
        print(f"  ... 외 {len(notes) - max_rows}개 노트")


# ═══════════════════════════════════════════════════════════
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="6키 자동 채보 생성기 v2")
    parser.add_argument("audio")
    parser.add_argument("--difficulty", choices=["easy", "medium", "hard"], default="medium")
    parser.add_argument("--output", default=None)
    args = parser.parse_args()

    output = args.output or (args.audio.rsplit(".", 1)[0] + ".chart.json")
    notes, meta = generate_chart(args.audio, args.difficulty)
    print_preview(notes)
    save_chart(notes, meta, output, args.audio, args.difficulty)