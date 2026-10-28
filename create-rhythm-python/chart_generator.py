"""
리듬게임 자동 채보 생성기 (1 / 2 / 4 / 6key)
──────────────────────────────────────────────────────────
처리 순서 (키 수와 무관하게 공통):
  오디오 로딩 → BPM 분석 → onset 감지 → [비트 격자 스냅] → [최소 간격 솎기]
  → 레인 배치 → JSON 저장

키 수별 차이:
  6key   : 난이도별 onset 감도(delta/wait). 레인은 주파수 기반 —
           저/고음 에너지 비율로 왼손(1~3)/오른손(4~6), 그룹 안 3등분 서브밴드 중
           에너지 최대 구간으로 레인 결정. 저/고음 경계와 3등분 경계는 곡마다 자동 분석.
           기본적으로 스냅·솎기 없음.
  1/2/4key : '쉬움' 채보. 레인은 주파수와 무관하게 배분 (1key는 모두 레인 1).
           1) 노트와 노트 사이 최소 간격 0.5초  (min_gap)
              최대 간격(max_gap)보다 긴 빈 구간은 노트를 채움 (무음 구간 제외)
           2) '동시 치기'(양손 동시)는 2key에만 존재, 4key에는 없음
           3) 같은 키 최대 3연속
           4) 모든 키의 사용 빈도가 비슷하도록 배분
           기본적으로 16분음표 격자에 스냅.

비트 격자 스냅:
  · onset(멜로디 음표 시작)을 곡의 비트/서브비트 격자점으로 옮겨
    실제 음보다 조금 빨리/늦게 잡히는 미세한 어긋남을 교정한다.
  · 격자에서 너무 멀리 떨어진 onset은 (옵션으로) 제외해
    "왜 여기서 노트가 나오지?" 하는 잘못 잡힌 노트를 줄일 수 있다.

사용법:
    python chart_generator.py <오디오파일> --keys 6 [--difficulty easy|medium|hard]
    python chart_generator.py <오디오파일> --keys 2 [--difficulty easy|normal]
    python chart_generator.py song.mp3 --keys 4 --output song_4k.json
    # 1key (게임 Single 모드용, 기본 출력 <곡>_single.json)
    python chart_generator.py song.mp3 --keys 1 --min-gap 1.25
    # 격자 조정 예: 8분음표 격자 + 격자에서 먼 노트 정리
    python chart_generator.py song.mp3 --keys 4 --subdiv 2 --snap-tolerance 0.3
    # 2분음표 / 온음표 격자 (마디 첫 박은 저음 onset + 화음 변화로 추정)
    python chart_generator.py song.mp3 --keys 2 --subdiv 0.5
    python chart_generator.py song.mp3 --keys 2 --subdiv 0.25
    # 추정한 첫 박이 틀렸으면 박 단위로 보정 (2·4박에 맞으면 1 또는 -1)
    python chart_generator.py song.mp3 --keys 2 --subdiv 0.5 --downbeat-shift 1
    # 신경망 비트 추적기 사용 (박자가 밀리는 곡, 3/4박자 곡에 권장)
    python chart_generator.py song.mp3 --keys 2 --subdiv 0.5 --beat-tracker beat_this

beat_this 설치 (선택, --beat-tracker beat_this 를 쓸 때만 필요):
    pip install torch torchaudio
    pip install https://github.com/CPJKU/beat_this/archive/main.zip
    # 스냅 끄기 / 6key에 스냅 켜기
    python chart_generator.py song.mp3 --keys 4 --subdiv 0
    python chart_generator.py song.mp3 --keys 6 --subdiv 4
    # 멜로디(하모닉) 성분만으로 onset 감지
    python chart_generator.py song.mp3 --keys 4 --harmonic

출력 JSON은 게임의 SongLoader가 읽는 형식과 동일하며 lane은 1-based로 저장한다.
difficulty는 "<keys>k-<난이도>", meta.seconds_per_beat는 한 박자 길이(60/BPM).
만든 파일은 Assets/Resources/Songs/ 에 넣는다.
"""

import os
import sys
import json
import random
import argparse

import numpy as np

try:
    import librosa
except ImportError:
    sys.exit("librosa가 없습니다. pip install numpy librosa 로 설치해 주세요.")


# ── 난이도 프리셋 ───────────────────────────────────────────
# 6key  — delta: onset 감지 문턱(낮을수록 노트 많음), wait: onset 사이 최소 대기(초)
# 1/2/4key — min_gap: 노트 사이 최소 간격(초), 클수록 쉬움. 하한 0.5.
#          max_gap: 노트 사이 최대 간격(초). 이보다 긴 빈 구간은 노트를 채워 넣는다.
#          chord_ratio: 2key 동시치기 비율(0~1). 4key에서는 무시.
DIFFICULTY = {
    6: {
        "easy":   {"delta": 0.12, "wait": 0.15},
        "medium": {"delta": 0.07, "wait": 0.10},
        "hard":   {"delta": 0.03, "wait": 0.05},
    },
    2: {
        "easy":   {"min_gap": 0.70, "max_gap": 3.0, "chord_ratio": 0.10},
        "normal": {"min_gap": 0.50, "max_gap": 2.5, "chord_ratio": 0.18},
    },
}
DIFFICULTY[1] = DIFFICULTY[4] = DIFFICULTY[2]
DEFAULT_DIFFICULTY = {6: "medium", 1: "easy", 2: "easy", 4: "easy"}
DEFAULT_SUBDIV     = {6: 0, 1: 4, 2: 4, 4: 4}   # 0 = 스냅 안 함
COARSE_SUBDIV      = (0.25, 0.5)           # 온음표 / 2분음표 격자

LITE_MIN_GAP  = 0.5   # 1/2/4key 최소 간격 하한(초)
FILL_DELTA    = 0.02  # 빈 구간 채우기용 onset 감지 문턱 (기본 0.07보다 민감)
FILL_MIN_RMS  = 0.3   # 음량이 곡 중앙값의 이 비율 미만인(거의 무음) 곳에는 채우지 않음
MIN_CHORD_GAP = 1.2   # 동시치기끼리 최소 간격(초): 너무 자주/연속으로 안 나오게
MAX_SAME_RUN  = 3     # 같은 키 최대 연속 횟수

HOP   = 512
N_FFT = 2048


# ═══════════════════════════════════════════════════════════
# 1. BPM 분석
# ═══════════════════════════════════════════════════════════

def analyze_bpm(y, sr):
    """tempo (BPM)와 beat 타이밍(초)을 반환."""
    tempo, beat_frames = librosa.beat.beat_track(y=y, sr=sr)
    beat_times = librosa.frames_to_time(beat_frames, sr=sr)
    bpm = float(tempo) if np.isscalar(tempo) else float(tempo[0])
    spb = 60.0 / bpm if bpm > 0 else 0.0   # seconds per beat
    print(f"  ↳ BPM: {bpm:.2f}  (한 박자 = {spb:.4f}초), 비트 {len(beat_times)}개")
    return bpm, spb, beat_times


def analyze_beats_beat_this(y, sr):
    """
    beat_this(CPJKU, 신경망 비트 추적기)로 비트와 마디 첫 박을 함께 찾는다.
    librosa beat_track보다 박자가 덜 밀리고, 3/4박자 등 4/4가 아닌 곡도 마디를 찾는다.
    첫 실행 때 모델 파일(final0)을 내려받는다. GPU가 있으면 GPU를 쓴다.
    반환: (bpm, spb, beat_times, downbeat_times)
    """
    try:
        import torch
        from beat_this.inference import Audio2Beats
    except ImportError:
        sys.exit("beat_this가 없습니다. 아래 두 줄로 설치해 주세요.\n"
                 "  pip install torch torchaudio\n"
                 "  pip install https://github.com/CPJKU/beat_this/archive/main.zip")

    device = "cuda" if torch.cuda.is_available() else "cpu"
    beats, downbeats = Audio2Beats(checkpoint_path="final0", device=device)(y, sr)
    beats     = np.asarray(beats, dtype=float)
    downbeats = np.asarray(downbeats, dtype=float)

    bpm = 60.0 / float(np.median(np.diff(beats))) if beats.size >= 2 else 0.0
    spb = 60.0 / bpm if bpm > 0 else 0.0
    print(f"  ↳ [beat_this/{device}] BPM: {bpm:.2f}  (한 박자 = {spb:.4f}초), "
          f"비트 {len(beats)}개, 마디 첫 박 {len(downbeats)}개")
    return bpm, spb, beats, downbeats


# ═══════════════════════════════════════════════════════════
# 2. 비트 격자 (beat_track → 서브비트로 세분화) & 스냅
# ═══════════════════════════════════════════════════════════

def clean_beats(beat_times, close_ratio=0.6, wide_ratio=1.5):
    """
    비트 추적 결과의 튀는 비트를 정리한다.
      · 직전 비트와 (간격 중앙값 × close_ratio)보다 가까운 비트: 가짜 비트가 끼어든 것.
        둘 중 '그 전 비트 + 한 박'에 더 가까운 쪽만 남긴다.
      · 간격이 (중앙값 × wide_ratio)보다 넓은 곳: 비트를 놓친 것. 사이를 고르게 채운다.
    가짜 비트가 하나만 있어도 그 뒤 마디 위치(1·3박)가 밀리므로 격자를 만들기 전에 정리한다.
    반환: (정리된 비트 배열, 뺀 개수, 채운 개수)
    """
    beats = np.asarray(beat_times, dtype=float)
    if beats.size < 3:
        return beats, 0, 0
    period = float(np.median(np.diff(beats)))
    if period <= 0:
        return beats, 0, 0

    kept, removed = [float(beats[0])], 0
    for b in beats[1:]:
        if b - kept[-1] >= close_ratio * period:
            kept.append(float(b))
            continue
        removed += 1
        ref = kept[-2] + period if len(kept) >= 2 else kept[-1]
        if abs(b - ref) < abs(kept[-1] - ref):
            kept[-1] = float(b)

    out, added = [kept[0]], 0
    for b in kept[1:]:
        gap = b - out[-1]
        if gap > wide_ratio * period:
            n = int(round(gap / period))
            for _ in range(n - 1):
                out.append(out[-1] + gap / n)
            added += n - 1
        out.append(b)
    return np.array(out), removed, added


def extend_beats(beat_times, duration):
    """
    beat_track이 찾은 비트를 곡 시작~끝 전체로 앞뒤 연장한다(비트 간격 중앙값 사용).
    비트가 너무 적으면 빈 배열을 반환한다(→ 스냅 생략).
    """
    beat_times = np.asarray(beat_times, dtype=float)
    if beat_times.size < 2:
        return np.array([])

    # 비트 간격(주기)의 중앙값 — 앞뒤 연장에 사용
    period = float(np.median(np.diff(beat_times)))
    if period <= 0:
        return np.array([])

    back = []
    t = beat_times[0] - period
    while t > -1e-9:
        back.append(t)
        t -= period
    fwd = []
    t = beat_times[-1] + period
    while t <= duration + 1e-9:
        fwd.append(t)
        t += period

    return np.concatenate([np.array(back[::-1]), beat_times, np.array(fwd)])


def estimate_downbeat(y, sr, beats, beats_per_bar=4):
    """
    마디 첫 박(강박)이 beats[p::beats_per_bar] 중 어느 p인지 추정한다.

    beat_track은 강박을 알려 주지 않으므로 두 단서를 합친다.
      · 저음(150Hz 미만) onset 세기 — 킥 드럼은 주로 1·3박에 온다.
        (전체 대역 onset은 2·4박의 스네어에 끌려가 오답을 내기 쉽다)
      · 화음 변화량 — 코드는 주로 마디 첫 박에서 바뀐다.
    각 단서를 위상별 평균 / 전체 평균으로 정규화해 더하고, 가장 큰 위상을 고른다.
    반환: (위상, 위상별 점수)
    """
    S      = np.abs(librosa.stft(y, n_fft=N_FFT, hop_length=HOP))
    freqs  = librosa.fft_frequencies(sr=sr, n_fft=N_FFT)
    frames = np.clip(librosa.time_to_frames(beats, sr=sr, hop_length=HOP), 0, S.shape[1] - 1)

    # ── 저음 onset: 비트 시각 ±2프레임 안의 최댓값 (비트 위치 오차 흡수) ──
    low_env = librosa.onset.onset_strength(
        S=librosa.power_to_db(S[freqs < 150.0] ** 2), sr=sr, hop_length=HOP)
    low = np.array([low_env[max(0, f - 2):f + 3].max() for f in frames])

    # ── 화음 변화: 비트 경계 앞뒤 구간의 chroma 코사인 거리 ─────────
    chroma = librosa.feature.chroma_stft(S=S ** 2, sr=sr, hop_length=HOP)
    seg = librosa.util.sync(chroma, frames, aggregate=np.median)   # 열 i+1 = beats[i]~beats[i+1]
    seg = seg / (np.linalg.norm(seg, axis=0, keepdims=True) + 1e-9)
    change = 1.0 - np.sum(seg[:, :-1] * seg[:, 1:], axis=0)        # change[i] = beats[i] 경계

    scores = np.zeros(beats_per_bar)
    for v in (low, change):
        if len(v) != len(beats) or v.mean() <= 0:
            continue
        scores += np.array([v[p::beats_per_bar].mean() for p in range(beats_per_bar)]) / v.mean()
    return int(np.argmax(scores)), scores


def bar_positions_from_downbeats(beats, downbeats, tol=0.07):
    """
    beat_this가 찾은 마디 첫 박으로 각 비트의 '마디 안 위치'(0 = 첫 박)를 매긴다.
    첫 박과 tol초 안에 있는 비트를 0으로 두고 그다음부터 1, 2, ...로 센다.
    첫 마디 이전(연장된 비트 포함)은 거꾸로 센다.
    반환: (positions, 마디당 박 수)
    """
    n = len(beats)
    if downbeats.size == 0:
        return np.arange(n) % 4, 4
    is_db = np.array([np.min(np.abs(downbeats - t)) < tol for t in beats])
    idx   = np.flatnonzero(is_db)
    if idx.size == 0:
        return np.arange(n) % 4, 4
    bpb = max(1, int(round(np.median(np.diff(idx))))) if idx.size >= 2 else 4

    pos = np.zeros(n, dtype=int)
    for i in range(idx[0]):
        pos[i] = (i - idx[0]) % bpb
    p = 0
    for i in range(idx[0], n):
        if is_db[i]:
            p = 0
        pos[i] = p % bpb   # 첫 박을 놓친 마디는 박 수로 되감음
        p += 1
    return pos, bpb


def build_grid(full_beats, subdiv, duration, positions=None):
    """
    연장된 비트(full_beats)를 기준으로 '스냅 격자'를 만든다.
      subdiv=0.25 → 온음표 격자 (마디 첫 박마다)
      subdiv=0.5  → 2분음표 격자 (1·3박. 3/4박자면 1·3박, 다음 마디에서 다시 1박부터)
      subdiv=1    → 정박(4분음표) 격자
      subdiv=2    → 8분음표 격자
      subdiv=4    → 16분음표 격자
    positions: 각 비트의 마디 안 위치(0 = 첫 박). subdiv < 1일 때만 쓴다.
    """
    if full_beats.size < 2:
        return np.array([])

    # 4분음표보다 굵은 격자: 마디 안 위치로 고름 (온음표 = 첫 박, 2분음표 = 첫 박부터 2박마다)
    if subdiv < 1:
        if positions is None:
            positions = np.arange(len(full_beats)) % 4
        keep = positions == 0 if subdiv < 0.5 else positions % 2 == 0
        grid = full_beats[keep]
        return np.array(sorted(g for g in grid if 0.0 <= g <= duration + 1e-9))

    # 인접 비트 사이를 subdiv등분
    subdiv = int(subdiv)
    grid = []
    for i in range(len(full_beats) - 1):
        a, b = full_beats[i], full_beats[i + 1]
        for s in range(subdiv):
            grid.append(a + (b - a) * s / subdiv)
    grid.append(full_beats[-1])

    grid = np.array(sorted(g for g in grid if 0.0 <= g <= duration + 1e-9))
    return grid


def snap_to_grid(times, grid, tol_ratio):
    """
    각 onset을 가장 가까운 격자점으로 옮긴다.
    격자 간격의 tol_ratio 배보다 멀리 떨어진 onset은 제외한다.

      · 균일 격자에서 '가장 가까운 격자점까지의 거리'는 항상 (간격/2) 이하다.
        따라서 tol_ratio=0.5 → 모든 노트 유지(순수 스냅, 제외 없음).
      · tol_ratio를 낮추면(예: 0.3) 격자 중간쯤 어정쩡하게 놓인 onset —
        박자에 잘 안 맞는, 잘못 잡혔을 가능성이 큰 노트 — 를 걸러낸다.

    반환: (스냅된 시각 리스트, 제외된 개수)
    """
    if grid.size == 0:
        return [round(float(t), 4) for t in times], 0

    spacing  = float(np.median(np.diff(grid)))
    max_dist = spacing * tol_ratio

    snapped, dropped = [], 0
    for t in times:
        idx = int(np.argmin(np.abs(grid - t)))
        g   = float(grid[idx])
        if abs(g - t) <= max_dist:
            snapped.append(round(g, 4))
        else:
            dropped += 1
    return snapped, dropped


# ═══════════════════════════════════════════════════════════
# 3. 타이밍 (onset 감지 → 격자 스냅 → 최소 간격 솎기)
# ═══════════════════════════════════════════════════════════

def detect_onsets(y, sr, keys, difficulty):
    """onset 프레임 감지. 6key는 난이도별 감도, 1/2/4key는 librosa 기본값."""
    if keys == 6:
        cfg = DIFFICULTY[6][difficulty]
        return librosa.onset.onset_detect(
            y=y, sr=sr,
            delta=cfg["delta"],
            wait=int(cfg["wait"] * sr / HOP),
            backtrack=True,
        )
    return librosa.onset.onset_detect(y=y, sr=sr, backtrack=True)


def build_times(onset_times, grid, snap_tol, min_gap):
    """
    onset 시각 → (grid가 있으면) 비트 격자에 스냅 → min_gap 이상 간격으로 솎는다.
    grid가 None이면 스냅하지 않고, min_gap이 0이면 솎지 않는다.
    """
    if grid is not None:
        snapped, dropped = snap_to_grid(onset_times, grid, snap_tol)
        times = sorted(set(snapped))   # 같은 격자점에 겹친 onset은 하나로 합침
        msg = f"  ↳ onset {len(onset_times)}개 → 스냅 후 {len(times)}개"
        if dropped:
            msg += f" (격자에서 먼 {dropped}개 제외)"
        if grid.size == 0:
            msg += "  [경고: 비트를 찾지 못해 스냅 생략]"
        print(msg)
    else:
        times = sorted(float(t) for t in onset_times)
        print(f"  ↳ onset {len(times)}개 (스냅 없음)")

    if min_gap <= 0:
        return times

    kept = []
    last = -1e9
    for t in times:
        if t - last >= min_gap:
            kept.append(round(float(t), 4))
            last = t
    return kept


def fill_gaps(times, y, sr, duration, grid, snap_tol, min_gap, max_gap, start=0.0):
    """
    노트 사이(곡 시작/끝 포함)가 max_gap초보다 긴 빈 구간에 노트를 채워 넣는다.
    음이 부드럽게 이어지는 곡은 onset이 적게 잡혀 긴 공백이 생기기 때문.

    후보: 더 민감하게(FILL_DELTA) 다시 감지한 onset (격자가 있으면 스냅) + 격자점.
    빈 구간마다 양옆 노트와 min_gap 이상 떨어진 후보 중 onset 세기가 가장 큰 것을 넣고,
    나뉜 구간이 max_gap 이하가 될 때까지 반복한다. 거의 무음인 곳은 후보에서 뺀다.
    start초 이전에는 채우지 않는다(곡 맨 앞 무시 구간).
    반환: (채운 시각 리스트, 추가한 개수)
    """
    env = librosa.onset.onset_strength(y=y, sr=sr, hop_length=HOP)
    rms = librosa.feature.rms(y=y, hop_length=HOP)[0]
    n   = min(len(env), len(rms))

    def frames(ts):
        return np.clip(librosa.time_to_frames(ts, sr=sr, hop_length=HOP), 0, n - 1)

    cand = librosa.frames_to_time(
        librosa.onset.onset_detect(onset_envelope=env, sr=sr, hop_length=HOP,
                                   delta=FILL_DELTA, backtrack=True), sr=sr, hop_length=HOP)
    if grid is not None and grid.size:
        cand, _ = snap_to_grid(cand, grid, snap_tol)
        cand = np.concatenate([np.asarray(cand, dtype=float), grid])
    cand = np.unique(np.round(np.asarray(cand, dtype=float), 4))
    cand = cand[(cand >= start) & (cand <= duration)]
    if cand.size == 0:
        return times, 0

    f     = frames(cand)
    score = np.array([env[max(0, i - 2):i + 3].max() for i in f])
    loud  = rms[f] >= FILL_MIN_RMS * np.median(rms)
    cand, score = cand[loud], score[loud]

    added = []

    def fill(a, b, a_note, b_note):
        if b - a <= max_gap:
            return
        lo = a + (min_gap if a_note else 0.0)
        hi = b - (min_gap if b_note else 0.0)
        m  = (cand >= lo) & (cand <= hi)
        if not m.any():
            return
        c = float(cand[m][np.argmax(score[m])])
        added.append(c)
        fill(a, c, a_note, True)
        fill(c, b, True, b_note)

    bounds = [start] + list(times) + [duration]
    for i in range(len(bounds) - 1):
        fill(bounds[i], bounds[i + 1], i > 0, i + 1 < len(bounds) - 1)

    return sorted(list(times) + added), len(added)


# ═══════════════════════════════════════════════════════════
# 4-A. 6key 레인 배치: 주파수 경계 자동 분석
# ═══════════════════════════════════════════════════════════

def analyze_boundaries(S, freqs, sr, onset_frames):
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


def get_lane(spectrum, freqs, low_high_boundary, low_bounds, high_bounds, prev_lane):
    """
    에너지 비율로 저/고음 그룹 결정 → 해당 그룹 내 서브밴드 에너지로 레인 결정.
    반환 레인은 1-based (1~6).
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
    else:
        bounds     = high_bounds
        base_lane  = 4

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


def assign_lanes_by_frequency(y, sr, times, onset_frames):
    """
    6key 레인 배치. 반환: ([(time, [lane0]), ...], 경계 meta)  (lane은 0-based)
    onset_frames는 경계 분석에 쓰는 프레임(= times에 대응).
    """
    S     = np.abs(librosa.stft(y, n_fft=N_FFT, hop_length=HOP))
    freqs = librosa.fft_frequencies(sr=sr, n_fft=N_FFT)

    low_high_boundary, low_bounds, high_bounds = analyze_boundaries(S, freqs, sr, onset_frames)

    events    = []
    prev_lane = -1
    for t in times:
        frame = librosa.time_to_frames(t, sr=sr, hop_length=HOP)
        frame = min(int(frame), S.shape[1] - 1)
        spectrum = S[:, frame]

        if spectrum.sum() < 1e-6:
            continue

        lane = get_lane(spectrum, freqs, low_high_boundary, low_bounds, high_bounds, prev_lane)
        events.append((t, [lane - 1]))
        prev_lane = lane

    meta = {
        "low_high_boundary": round(low_high_boundary, 2),
        "low_bounds":        [round(v, 2) for v in low_bounds],
        "high_bounds":       [round(v, 2) for v in high_bounds],
    }
    return events, meta


# ═══════════════════════════════════════════════════════════
# 4-B. 1/2/4key 레인 배치 (빈도 균등 + 같은 키 3연속 제한 + 2key 동시치기)
# ═══════════════════════════════════════════════════════════

def assign_lanes_balanced(times, keys, chord_ratio, seed=0):
    """
    times   : 정렬된 노트 시각 리스트
    keys    : 1, 2 또는 4  (1이면 모두 레인 0)
    반환    : ([(time, [lane0, lane1, ...]), ...], counts)  (lane은 0-based)

    규칙:
      - 매 노트는 '지금까지 가장 적게 쓴 레인'을 골라 빈도를 균등화한다.
      - 같은 레인이 3번 연속되면 다음엔 다른 레인을 강제한다.
      - keys==2이고 조건을 만족하면 두 레인을 동시에 치는 '동시치기'를 넣는다.
        (동시치기는 두 레인 카운트를 모두 올리고, 단일 연속 streak을 리셋한다)
    """
    rng      = random.Random(seed)
    counts   = [0] * keys
    result   = []
    run_lane = -1     # 현재 연속 중인 단일 레인
    run_len  = 0
    last_chord_t = -1e9

    for t in times:
        # ── 2key 동시치기 판정 ───────────────────────────────
        if keys == 2 and chord_ratio > 0.0 \
                and (t - last_chord_t) >= MIN_CHORD_GAP \
                and rng.random() < chord_ratio:
            counts[0] += 1
            counts[1] += 1
            run_lane, run_len = -1, 0
            last_chord_t = t
            result.append((t, [0, 1]))
            continue

        # ── 단일 노트: 가장 적게 쓴 레인 우선, 3연속이면 제외 ──
        order  = sorted(range(keys), key=lambda l: (counts[l], rng.random()))
        chosen = next((l for l in order
                       if not (l == run_lane and run_len >= MAX_SAME_RUN)),
                      order[0])

        counts[chosen] += 1
        if chosen == run_lane:
            run_len += 1
        else:
            run_lane, run_len = chosen, 1

        result.append((t, [chosen]))

    return result, counts


# ═══════════════════════════════════════════════════════════
# 5. 채보 생성 메인
# ═══════════════════════════════════════════════════════════

def generate_chart(audio_path, keys, difficulty, subdiv=None, snap_tol=0.5,
                   harmonic=False, min_gap=None, chord_ratio=None, seed=0,
                   downbeat_shift=0, beat_tracker="librosa", max_gap=None, skip_start=0.5):
    """반환: (events, meta) — events는 [(time, [lane0, ...]), ...] (lane 0-based)."""
    cfg = DIFFICULTY[keys][difficulty]
    if subdiv is None:
        subdiv = DEFAULT_SUBDIV[keys]
    if keys == 6:
        min_gap = max(0.0, min_gap or 0.0)
        max_gap = max(0.0, max_gap or 0.0)
    else:
        min_gap = max(LITE_MIN_GAP, min_gap if min_gap is not None else cfg["min_gap"])
        max_gap = max(0.0, max_gap if max_gap is not None else cfg["max_gap"])
        chord_ratio = chord_ratio if chord_ratio is not None else cfg["chord_ratio"]

    print(f"[1/5] 오디오 로딩: {audio_path}")
    y, sr    = librosa.load(audio_path, sr=None, mono=True)
    duration = librosa.get_duration(y=y, sr=sr)

    # 멜로디 강조(선택): 하모닉 성분만 뽑아 타악기/저음 onset을 억제
    analysis = librosa.effects.harmonic(y) if harmonic else y

    print("[2/5] 박자 분석...")
    if beat_tracker == "beat_this":
        bpm, spb, beat_times, downbeat_times = analyze_beats_beat_this(y, sr)
    else:
        bpm, spb, beat_times = analyze_bpm(y, sr)
    beat_times, n_removed, n_added = clean_beats(beat_times)
    if n_removed or n_added:
        print(f"  ↳ 비트 정리: 튀는 비트 {n_removed}개 제거, 빠진 비트 {n_added}개 보충")
    grid = None
    if subdiv > 0:
        full_beats = extend_beats(beat_times, duration)
        positions = None
        if subdiv < 1 and full_beats.size >= 2:
            if beat_tracker == "beat_this":
                positions, bpb = bar_positions_from_downbeats(full_beats, downbeat_times)
                print(f"  ↳ 박자표: {bpb}/4 (beat_this 마디 첫 박 기준)")
            if beat_tracker != "beat_this" or bpb < 2:
                if beat_tracker == "beat_this":
                    print("  ↳ [경고] beat_this가 마디를 찾지 못함 → 저음/화음 변화로 추정")
                auto, scores = estimate_downbeat(y, sr, full_beats)
                positions, bpb = (np.arange(len(full_beats)) - auto) % 4, 4
                print(f"  ↳ 강박 추정: 위상 {auto} (점수 {' '.join(f'{v:.2f}' for v in scores)}), 4/4 가정")
            if downbeat_shift:
                positions = (positions - downbeat_shift) % bpb
                print(f"  ↳ 수동 보정 {downbeat_shift:+d}박")
            firsts = full_beats[positions == 0][:4]
            print(f"  ↳ 마디 첫 박: {', '.join(f'{t:.2f}s' for t in firsts)} ...")
        grid = build_grid(full_beats, subdiv, duration, positions)
        print(f"  ↳ 격자 {grid.size}점 (subdiv={subdiv:g})")

    print(f"[3/5] Onset 감지 (min_gap={min_gap:.2f}s"
          + (f", snap_tol={snap_tol}" if grid is not None else "") + ")...")
    onset_frames = detect_onsets(analysis, sr, keys, difficulty)
    onset_times  = librosa.frames_to_time(onset_frames, sr=sr)
    times        = build_times(onset_times, grid, snap_tol, min_gap)
    if skip_start > 0:
        n_before = len(times)
        times = [t for t in times if t >= skip_start]
        if len(times) < n_before:
            print(f"  ↳ 처음 {skip_start:g}초 안의 노트 {n_before - len(times)}개 무시")
    if max_gap > 0:
        times, n_added = fill_gaps(times, analysis, sr, duration, grid, snap_tol,
                                   min_gap, max_gap, start=max(0.0, skip_start))
        gaps = np.diff([max(0.0, skip_start)] + list(times) + [duration])
        print(f"  ↳ 빈 구간 채우기 (max_gap={max_gap:.2f}s): {n_added}개 추가, "
              f"남은 최대 공백 {gaps.max():.1f}s")
    print(f"  ↳ 최종 노트 시각 {len(times)}개")

    meta = {"bpm": bpm, "seconds_per_beat": spb}

    if keys == 6:
        print("[4/5] 주파수 경계 자동 분석 & 레인 배치...")
        # 스냅/솎기를 하지 않았다면 원래 onset 프레임 그대로 경계를 분석
        if grid is None and min_gap <= 0 and len(times) == len(onset_frames):
            frames = onset_frames
        else:
            frames = librosa.time_to_frames(times, sr=sr, hop_length=HOP)
        events, bound_meta = assign_lanes_by_frequency(y, sr, times, frames)
        meta.update(bound_meta)
    else:
        print(f"[4/5] 레인 배분 ({keys}key)...")
        events, _ = assign_lanes_balanced(times, keys,
                                          chord_ratio if keys == 2 else 0.0, seed)

    return events, meta


# ═══════════════════════════════════════════════════════════
# 6. 저장 & 미리보기
# ═══════════════════════════════════════════════════════════

def to_notes(events):
    """(time, [lanes]) → JSON용 노트 리스트. lane을 1-based로 변환."""
    notes = []
    for t, lanes in events:
        for lane in lanes:
            notes.append({"time": round(float(t), 4), "lane": lane + 1})
    return notes


def save_chart(notes, meta, audio_path, keys, difficulty, output_path):
    bpm = meta["bpm"]
    data = {
        "source":     os.path.basename(audio_path),   # 같은 곡의 키별 채보가 오디오를 공유
        "difficulty": f"{keys}k-{difficulty}",
        "meta": {
            **meta,
            "bpm":              round(float(bpm), 4),
            "seconds_per_beat": round(60.0 / bpm, 6) if bpm > 0 else 0.0,
        },
        "notes": notes,
    }
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
    print(f"\n채보 저장 완료: {output_path}  ({len(notes)}노트)")


def print_summary(events, keys):
    counts = [0] * keys
    for _, lanes in events:
        for l in lanes:
            counts[l] += 1
    chords = sum(1 for _, l in events if len(l) > 1)
    print("\n── 요약 ─────────────────────────────────")
    print(f"  이벤트 {len(events)}개 (그 중 동시치기 {chords}개)")
    for i, c in enumerate(counts):
        print(f"    레인 {i + 1}: {c}회")
    # 최대 연속 검증
    run_lane, run_len, max_run = -1, 0, 0
    for _, lanes in events:
        if len(lanes) != 1:
            run_lane, run_len = -1, 0
            continue
        l = lanes[0]
        run_len = run_len + 1 if l == run_lane else 1
        run_lane = l
        max_run = max(max_run, run_len)
    if keys > 1:
        print(f"  같은 키 최대 연속: {max_run}" + (f" (제한 {MAX_SAME_RUN})" if keys != 6 else ""))


def print_preview(events, keys, max_rows=40):
    half = keys // 2
    print("\n── 채보 미리보기 ─────────────────────────────────")
    sep  = " | " if half else ""
    print("  시간(초)   레인   " + "  ".join(str(i + 1) for i in range(half))
          + sep + "  ".join(str(i + 1) for i in range(half, keys)))
    print("  " + "-" * 40)
    for t, lanes in events[:max_rows]:
        cols = ["·"] * keys
        for l in lanes:
            cols[l] = "■"
        label = "+".join(str(l + 1) for l in lanes)
        print(f"  {t:7.3f}s  [{label}]   {'  '.join(cols[:half])}{sep}{'  '.join(cols[half:])}")
    if len(events) > max_rows:
        print(f"  ... 외 {len(events) - max_rows}개 이벤트")


# ═══════════════════════════════════════════════════════════
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="리듬게임 자동 채보 생성기 (1/2/4/6key)")
    parser.add_argument("audio")
    parser.add_argument("--keys", type=int, choices=[1, 2, 4, 6], default=1,
                        help="1=Single(모두 레인 1), 2/4=쉬움, 6=주파수 기반")
    parser.add_argument("--difficulty", choices=["easy", "normal", "medium", "hard"], default=None,
                        help="6key: easy|medium|hard (기본 medium), 1/2/4key: easy|normal (기본 easy)")
    parser.add_argument("--min-gap", type=float, default=None,
                        help="노트 최소 간격(초). 1/2/4key는 미지정 시 난이도 기본값(하한 0.5), "
                             "6key는 기본 0(솎지 않음)")
    parser.add_argument("--skip-start", type=float, default=0.5,
                        help="곡 맨 처음 이 시간(초) 안의 노트는 만들지 않음. 0=끔. 기본 0.5")
    parser.add_argument("--max-gap", type=float, default=None,
                        help="노트 최대 간격(초). 이보다 긴 빈 구간은 노트를 채움(무음 구간 제외). "
                             "0=끔. 1/2/4key 기본 easy 3.0 / normal 2.5, 6key 기본 0")
    parser.add_argument("--chord-ratio", type=float, default=None,
                        help="2key 동시치기 비율(0~1). 미지정 시 난이도 기본값")
    parser.add_argument("--subdiv", type=float, default=0.5,
                        help="비트 격자 세분화. 0=스냅 안 함, 0.25=온음표, 0.5=2분음표, "
                             "1=정박(4분음표), 2=8분음표, 4=16분음표. 기본: 1/2/4key 4, 6key 0")
    parser.add_argument("--beat-tracker", choices=["librosa", "beat_this"], default="beat_this",
                        help="비트/마디 추적기. beat_this는 신경망 모델로 더 정확하고 3/4박자도 처리 "
                             "(torch, beat_this 설치 필요). 기본 librosa")
    parser.add_argument("--downbeat-shift", type=int, default=0,
                        help="2분음표/온음표 격자에서 자동 추정한 마디 첫 박을 N박 뒤로 민다(음수면 앞으로). "
                             "격자가 2·4박에 맞으면 1 또는 -1")
    parser.add_argument("--snap-tolerance", type=float, default=0.5,
                        help="격자에서 이 비율(격자간격 대비) 이상 떨어진 onset은 제외. "
                             "0.5=모두 유지(순수 스냅), 낮출수록 엄격(예 0.3)")
    parser.add_argument("--harmonic", action="store_true",
                        help="하모닉(멜로디) 성분만으로 onset 감지 — 타악기/저음 노트를 줄임")
    parser.add_argument("--seed", type=int, default=0, help="2/4key 레인 배분 난수 시드")
    parser.add_argument("--output", default=None)
    args = parser.parse_args()

    difficulty = args.difficulty or DEFAULT_DIFFICULTY[args.keys]
    if difficulty not in DIFFICULTY[args.keys]:
        parser.error(f"{args.keys}key 난이도는 {'|'.join(DIFFICULTY[args.keys])} 중 하나입니다.")
    if args.subdiv is not None and not (args.subdiv in COARSE_SUBDIV
                                        or (args.subdiv >= 0 and args.subdiv == int(args.subdiv))):
        parser.error("--subdiv는 0, 0.25, 0.5 또는 양의 정수여야 합니다.")

    events, meta = generate_chart(
        args.audio, args.keys, difficulty,
        subdiv=args.subdiv, snap_tol=args.snap_tolerance, harmonic=args.harmonic,
        min_gap=args.min_gap, chord_ratio=args.chord_ratio, seed=args.seed,
        downbeat_shift=args.downbeat_shift, beat_tracker=args.beat_tracker,
        max_gap=args.max_gap, skip_start=args.skip_start,
    )

    base   = args.audio.rsplit(".", 1)[0]
    suffix = {6: ".chart.json", 1: "_single.json"}.get(args.keys, f"_{args.keys}k.json")
    output = args.output or (base + suffix)

    print("[5/5] 저장...")
    print_preview(events, args.keys)
    print_summary(events, args.keys)
    save_chart(to_notes(events), meta, args.audio, args.keys, difficulty, output)
