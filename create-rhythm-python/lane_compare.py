"""
레인 배치 비교 플레이어 (STFT 방식 vs 퍼센타일 방식 / 멜 필터 방식) — 2 / 4 / 6key
──────────────────────────────────────────────────────────
chart_generator.py는 그대로 두고, 레인 배치 방식만 비교하기 위한 단순화 버전.
음원 하나를 넣으면 두 방식의 채보를 좌우로 나란히 놓고 음악과 함께 재생한다.

공통 처리 (둘 다 똑같이):
  오디오 로딩 → onset 감지 → 레인 배치 → 나란히 재생
  · onset 감지는 기본적으로 chart_generator.py --difficulty easy와 같은 방식
    (librosa 기본 감도). 단, easy의 최소 간격(0.7초) 솎기는 하지 않음
  · 비트 추적 / 격자 스냅 / 비트 정리 / 최소 간격 / 빈 구간 채우기 없음
  · 노트 최소 간격 (--min-gap, 기본 0.25초): 레인 배치 '전에' onset 시각 자체를 솎는다.
    직전에 남긴 시각과 이 시간보다 가까운 시각은 버린다 (키와 무관).
    두 방식이 정확히 같은 시각을 받으므로, 화면의 차이는 키 배치 차이뿐이다. 0이면 끔.

레인 배치 방식:
  (1) stft : chart_generator.py의 6key 방식 그대로 (왼쪽 화면).
             선형 STFT 진폭에서 저음/고음 에너지를 비교해 왼손/오른손 그룹을 정하고,
             그룹 안 서브밴드 중 에너지 최대 구간을 레인으로 한다.
             그룹 안 칸 수 = 키 수의 절반 (6key 3칸, 4key 2칸, 2key 나누지 않음).
             경계(저/고음 경계, 3등분 경계)는 곡마다 자동 분석.
  (2) quantile (--method2 quantile) : 필터 없이 퍼센타일로 바로 나눈다 (오른쪽 화면).
             onset마다 '대표 주파수' 하나를 구한다:
               --q-spec flux(기본) : 어택 직후 파워 − 어택 직전 파워 (늘어난 부분만) 의
                                     누적 50% 주파수 → 계속 울리던 배경 소리의 영향을 줄임
               --q-spec frame      : onset 프레임 파워 그대로의 누적 50% 주파수
             (--q-fmin ~ --q-fmax 범위의 bin만 사용, 기본 40~8000Hz)
             곡 전체 onset의 대표 주파수 분포를 KEYS구간으로 나눈 경계로 레인을 정한다.
               --q-bounds gap(기본) : 노트 수가 비슷하되, 경계를 소리 무리 사이 빈틈에 둔다
                                      (한 소리가 두 레인으로 쪼개지지 않게). --q-balance로 조절
               --q-bounds exact     : 퍼센타일 정확히 등분 (4key → 25/50/75%)
             대표 주파수가 같으면 항상 같은 레인(결정적 함수), 높을수록 오른쪽. 동시치기 없음.
             ※ 퍼센타일은 순서만 보므로 Hz로 나누든 멜로 나누든 결과가 같다.
  (2) mel  (기본, --method2 mel) : 키마다 멜 필터(삼각형 대역 필터)를 하나씩 둔다 (4key → 필터 4개) (오른쪽 화면).
             필터 범위 --mel-range auto(기본): onset마다 대표 주파수(누적 50% 지점)를 구하고,
             그 분포의 5~95% 퍼센타일(--mel-pct)에 첫 필터 / 마지막 필터의 꼭짓점이 오도록
             범위를 곡마다 맞춘다. (fixed면 --mel-fmin ~ --mel-fmax 그대로)
             onset 순간의 파워(--mel-spec flux(기본): 어택에서 늘어난 파워만, frame: 프레임 파워)에
             각 필터를 곱해 필터별 에너지를 구한다.
             레인마다 따로 판단: 필터 에너지 ÷ 그 필터의 평소값(곡 전체 onset 중앙값)이
             --mel-threshold(기본 2.0) 이상이면 그 레인에 노트를 만든다.
             여러 필터가 넘으면 동시치기. 하나도 안 넘으면 평소 대비 가장 큰 필터 하나에
             노트를 만든다 (노트 시각은 두 방식이 항상 같음 — 키 배치만 비교).

재생 화면:
  왼쪽 (1), 오른쪽 (2). 두 방식이 다른 레인을 고른 노트는 흰 테두리로 표시.
  동시치기 노트는 가로선으로 이어서 표시.
  조작: SPACE 일시정지 | ← → 5초 이동 | ↑ ↓ 노트 속도 | Q/ESC 종료
  터미널에는 두 방식의 레인을 나란히 비교한 표와 요약이 출력된다.

노트 시각:
  · 오디오 파일을 주면 onset 감지로 노트 시각을 정한다.
  · 채보 JSON을 주면 그 채보의 노트 시각을 그대로 쓰고(onset 감지 생략), 오디오는
    채보의 source 필드에서 찾는다. 예: chart_generator.py로 만든 _single.json의 타이밍에
    두 방식으로 6key 레인을 붙여 비교.

사용법:
    python lane_compare.py <오디오파일>
    python lane_compare.py songs/little_snowman_single.json     # 기존 채보 타이밍 사용
    python lane_compare.py songs/kite.mp3 --keys 6
    python lane_compare.py songs/kite.mp3 --onset 6k-hard
    python lane_compare.py songs/kite.mp3 --mel-pct 10        # 멜 필터 범위를 10~90% 퍼센타일로
    python lane_compare.py songs/kite.mp3 --mel-range fixed --mel-fmin 60 --mel-fmax 6000
    python lane_compare.py songs/kite.mp3 --method2 quantile  # 오른쪽을 퍼센타일 방식으로 (동시치기 없음)
    python lane_compare.py songs/kite.mp3 --mel-threshold 3   # 노트를 더 적게 (평소의 3배 넘을 때만)
    python lane_compare.py songs/kite.mp3 --png          # 비교 그림도 저장 (matplotlib 필요)
    python lane_compare.py songs/kite.mp3 --no-play      # 재생 없이 터미널 비교만
"""

import os
import sys
import json
import argparse

import numpy as np

try:
    import librosa
except ImportError:
    sys.exit("librosa가 없습니다. pip install numpy librosa 로 설치해 주세요.")


KEYS  = 4          # 기본 키 수 (2 / 4 / 6). 실행 시 --keys 로 바꿀 수 있음
HOP   = 512
N_FFT = 2048

# onset 감지 방식
#   easy      : chart_generator.py --difficulty easy (1/2/4key)와 같음 — librosa 기본 감도 (기본값)
#   6k-easy / 6k-medium / 6k-hard : chart_generator.py 6key 난이도별 감도
#               delta: 감지 문턱(낮을수록 노트 많음), wait: onset 사이 최소 대기(초)
ONSET_PRESETS = {
    "easy":      None,
    "6k-easy":   {"delta": 0.12, "wait": 0.15},
    "6k-medium": {"delta": 0.07, "wait": 0.10},
    "6k-hard":   {"delta": 0.03, "wait": 0.05},
}

# 오른쪽 화면 (2) 방식: "quantile"(퍼센타일) 또는 "mel"(멜 필터). 실행 시 --method2 로 바꿈
METHOD2 = "mel"


def m2_name():
    """(2) 방식의 짧은 이름 (표/범례용)."""
    return "퍼센타일" if METHOD2 == "quantile" else "멜 필터"


def m2_label():
    """(2) 방식 설명 (화면/그림 제목용)."""
    if METHOD2 == "quantile":
        return f"대표 주파수 퍼센타일 {KEYS}등분"
    return "키당 멜 필터 1개, 필터마다 threshold 넘으면 노트"


# 레인 색 (그림용, 레인 1~6 고정 순서)
LANE_COLORS = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300"]


# ═══════════════════════════════════════════════════════════
# 공통: onset 감지
# ═══════════════════════════════════════════════════════════

def detect_onsets(y, sr, preset):
    """onset 프레임과 시각(초)을 반환. preset은 ONSET_PRESETS의 키."""
    cfg = ONSET_PRESETS[preset]
    if cfg is None:
        frames = librosa.onset.onset_detect(y=y, sr=sr, backtrack=True)
    else:
        frames = librosa.onset.onset_detect(
            y=y, sr=sr, hop_length=HOP,
            delta=cfg["delta"],
            wait=int(cfg["wait"] * sr / HOP),
            backtrack=True,
        )
    return frames, librosa.frames_to_time(frames, sr=sr, hop_length=HOP)


def load_chart_times(chart_path):
    """
    기존 채보 JSON(chart_generator.py 출력 등)의 노트 시각과 오디오 경로를 읽는다.
    같은 시각의 노트(동시치기)는 하나로 합친다. 오디오는 채보와 같은 폴더의 source 파일.
    """
    with open(chart_path, encoding="utf-8-sig") as f:
        data = json.load(f)
    times = np.array(sorted({float(n["time"]) for n in data.get("notes", [])}))
    audio = os.path.join(os.path.dirname(os.path.abspath(chart_path)), data.get("source", ""))
    return times, audio, data.get("difficulty", "")


# ═══════════════════════════════════════════════════════════
# (1) STFT 방식 — chart_generator.py 6key 로직 (연속 방지 제외), 키 수에 맞게 일반화
# ═══════════════════════════════════════════════════════════

def stft_label():
    """화면/그림 제목용 (1) 방식 설명."""
    return "저/고음 그룹" if KEYS == 2 else f"저/고음 그룹 → 그룹 안 {KEYS // 2}칸"


def group_split():
    """
    한 그룹(저음/고음)을 몇 칸으로, 어느 퍼센타일에서 나눌지.
    6key는 chart_generator.py와 똑같이 3칸(33/66), 4key는 2칸(50), 2key는 나누지 않음.
    """
    n = KEYS // 2
    pcts = [33, 66] if n == 3 else list(np.linspace(0, 100, n + 1)[1:-1])
    return n, pcts


def stft_boundaries(S, freqs, sr, onset_frames):
    """
    저/고음 경계: 곡 평균 스펙트럼의 누적 에너지 50% 지점 (50~800Hz로 제한)
    그룹 안 경계: onset마다 그룹 안 무게중심 주파수를 모아 퍼센타일로 나눔
                  (6key 33/66, 4key 50, 2key 없음)
    반환: (low_high_boundary, low_bounds[n+1], high_bounds[n+1])  n = KEYS // 2
    """
    n, pcts = group_split()
    cum = np.cumsum(S.mean(axis=1))
    boundary = float(np.clip(freqs[np.searchsorted(cum, cum[-1] * 0.5)], 50.0, 800.0))
    low_mask, high_mask = freqs < boundary, freqs >= boundary

    low_c, high_c = [], []
    for f in onset_frames:
        spec = S[:, min(int(f), S.shape[1] - 1)]
        for mask, out in ((low_mask, low_c), (high_mask, high_c)):
            e = spec[mask]
            if e.sum() > 1e-6:
                out.append(float(np.sum(freqs[mask] * e) / e.sum()))

    fracs = [p / 100 for p in pcts]        # 후보가 부족할 때 쓰는 균등 분할 비율
    if len(low_c) >= 3:
        low_mid = [float(v) for v in np.percentile(low_c, pcts)] if pcts else []
    else:
        low_mid = [boundary * r for r in fracs]
    if len(high_c) >= 3:
        high_mid = [float(v) for v in np.percentile(high_c, pcts)] if pcts else []
    else:
        high_mid = [boundary + (sr / 2 - boundary) * r for r in fracs]

    low_bounds  = [0.0] + low_mid + [boundary]
    high_bounds = [boundary] + high_mid + [sr / 2]
    return boundary, low_bounds, high_bounds


def stft_lane(spec, freqs, boundary, low_bounds, high_bounds):
    """저/고음 에너지 비교로 그룹 → 그룹 안 서브밴드 에너지 최대 → 레인(1~KEYS)."""
    n = KEYS // 2
    is_low = spec[freqs < boundary].sum() >= spec[freqs >= boundary].sum()
    bounds, base = (low_bounds, 1) if is_low else (high_bounds, n + 1)
    sub = [spec[(freqs >= bounds[i]) & (freqs < bounds[i + 1])].sum() for i in range(n)]
    return base + int(np.argmax(sub))


def assign_stft(S, freqs, sr, onset_frames):
    """반환: (레인 행렬[onset, KEYS] — 노트마다 레인 1개, info)"""
    boundary, low_bounds, high_bounds = stft_boundaries(S, freqs, sr, onset_frames)
    M = np.zeros((len(onset_frames), KEYS), dtype=bool)
    for i, f in enumerate(onset_frames):
        spec = S[:, min(int(f), S.shape[1] - 1)]
        if spec.sum() >= 1e-6:
            M[i, stft_lane(spec, freqs, boundary, low_bounds, high_bounds) - 1] = True
    info = {
        "low_high_boundary": round(boundary, 2),
        "low_bounds":  [round(v, 2) for v in low_bounds],
        "high_bounds": [round(v, 2) for v in high_bounds],
    }
    return M, info


# ═══════════════════════════════════════════════════════════
# (2) 퍼센타일 방식 — 필터 없이 대표 주파수 분포를 KEYS등분
# ═══════════════════════════════════════════════════════════

def onset_power(S, onset_frames, spec="flux"):
    """
    노트마다 레인 판단에 쓸 파워 [bin, 노트].
      flux  : 노트 직후 파워 − 노트 직전 파워 (음수는 0) → 새로 친 소리만 남는다
      frame : 노트 프레임 파워 그대로
    """
    P = S ** 2
    f = np.clip(np.asarray(onset_frames, dtype=int), 0, P.shape[1] - 1)
    if spec == "frame":
        return P[:, f]
    n = P.shape[1]
    after  = np.max([P[:, np.minimum(f + d, n - 1)] for d in range(4)], axis=0)  # 직후 4프레임 중 최대
    before = P[:, np.maximum(f - 2, 0)]                                         # 직전
    return np.maximum(after - before, 0.0)


def onset_rep_freq(S, freqs, onset_frames, fmin, fmax, spec="flux"):
    """노트마다 대표 주파수 = 파워를 낮은 쪽부터 쌓아 절반이 되는 주파수. 소리가 없으면 NaN."""
    band = (freqs >= fmin) & (freqs <= fmax)
    cum  = np.cumsum(onset_power(S, onset_frames, spec)[band], axis=0)
    rep  = freqs[band][np.argmax(cum >= cum[-1] / 2, axis=0)]
    return np.where(cum[-1] > 0, rep, np.nan)


def auto_mel_centers(rep, keys, pct=5.0, mode="quantile", min_gap_mel=0.5):
    """
    곡에 맞춘 멜 필터 꼭짓점(Hz) keys개.
    양 끝 꼭짓점 = onset 대표 주파수 분포의 pct / (100-pct) 퍼센타일
    (최대/최소 그대로 쓰면 튀는 onset 하나에 범위가 끌려가므로 퍼센타일 사용).
      mode="even"     : 그 범위를 멜 스케일로 등간격 (범위만 맞춤)
      mode="quantile" : 꼭짓점도 분포의 퍼센타일 (pct ~ 100-pct 를 keys개로 등분한 지점)
                        → 노트가 몰린 대역은 필터가 촘촘해져 서로 다른 음이 같은 필터로 뭉치지 않음
    꼭짓점끼리 min_gap_mel 이상 떨어지게 한다 (같은 값이 겹치면 필터가 0폭이 되므로).
    """
    rep = rep[~np.isnan(rep)]
    if rep.size < 2:
        rep = np.array([40.0, 8000.0])
    m = librosa.hz_to_mel(rep)
    if mode == "quantile":
        c = np.percentile(m, np.linspace(pct, 100 - pct, keys))
    else:
        lo, hi = np.percentile(m, [pct, 100 - pct])
        c = np.linspace(lo, hi, keys)
    for k in range(1, keys):
        c[k] = max(c[k], c[k - 1] + min_gap_mel)
    return librosa.mel_to_hz(c)


def tri_filterbank(freqs, centers_hz):
    """
    꼭짓점(Hz)을 직접 받아 만드는 삼각형 필터뱅크 [len(centers), bin].
    필터 k: 멜 스케일에서 꼭짓점 k-1 → k 오르막, k → k+1 내리막 (이웃과 절반씩 겹침).
    양 끝 필터의 바깥쪽 발은 안쪽 간격만큼 대칭으로 뻗는다 (0Hz / 나이퀴스트에서 잘려도 꼭짓점은 그대로).
    면적 정규화(Slaney식, 2 / 폭Hz)를 적용한다.
    """
    c  = librosa.hz_to_mel(np.asarray(centers_hz, dtype=float))
    lf = np.r_[2 * c[0] - c[1], c[:-1]]
    rt = np.r_[c[1:], 2 * c[-1] - c[-2]]
    fm = librosa.hz_to_mel(freqs)[None, :]
    up   = (fm - lf[:, None]) / (c - lf)[:, None]
    down = (rt[:, None] - fm) / (rt - c)[:, None]
    W = np.maximum(0.0, np.minimum(up, down))
    width = librosa.mel_to_hz(rt) - librosa.mel_to_hz(lf)
    return W * (2.0 / np.maximum(width, 1e-9))[:, None]


def gap_bounds(vals, keys, balance=1.0):
    """
    정렬한 대표 주파수(로그)를 keys구간으로 나누는 경계를 동적계획법으로 고른다.
      점수 = Σ 경계에서의 빈틈 크기(전체 범위 대비) − balance × Σ (구간 노트 수 / 목표 − 1)²
    빈틈(서로 다른 소리 무리 사이)에 경계를 두면 한 소리가 두 레인으로 쪼개지지 않는다.
    balance가 클수록 노트 수 균등(퍼센타일 등분)에 가까워지고, 0이면 빈틈만 본다.
    반환: 경계 주파수 keys-1개 (Hz, 오름차순)
    """
    x = np.sort(np.log(vals))
    n = len(x)
    span = max(x[-1] - x[0], 1e-9)
    gn = np.zeros(n)
    gn[1:] = (x[1:] - x[:-1]) / span                   # gn[a] = x[a-1]과 x[a] 사이 빈틈
    target = n / keys
    sizes = np.arange(n + 1)
    pen = balance * (sizes / target - 1.0) ** 2        # 구간 크기별 벌점

    best = np.full((keys + 1, n + 1), -np.inf)
    back = np.zeros((keys + 1, n + 1), dtype=int)
    best[1, 1:] = -pen[1:]
    for k in range(2, keys + 1):
        for b in range(k, n + 1):
            a = np.arange(k - 1, b)                    # 마지막 구간 = [a, b)
            s = best[k - 1, a] + gn[a] - pen[b - a]
            j = int(np.argmax(s))
            best[k, b], back[k, b] = s[j], a[j]

    cuts, b = [], n
    for k in range(keys, 1, -1):
        b = back[k, b]
        cuts.append(b)
    return sorted(float(np.exp((x[a - 1] + x[a]) / 2)) for a in cuts)


def assign_quantile(S, freqs, onset_frames, fmin, fmax, spec="flux", bounds="gap", balance=1.0):
    """
    대표 주파수 분포를 KEYS구간으로 나눈 경계로 레인을 정한다.
      bounds="exact" : 퍼센타일 등분 (노트 수 정확히 균등, 한 소리가 쪼개질 수 있음)
      bounds="gap"   : 균등에 가깝되 소리 무리 사이 빈틈에 경계를 둠 (gap_bounds)
    lane = 대표 주파수가 들어가는 구간 번호 → 같은 주파수면 항상 같은 레인, 높을수록 오른쪽.
    반환: (레인 행렬[onset, KEYS], 대표 주파수[onset], info)
    """
    rep   = onset_rep_freq(S, freqs, onset_frames, fmin, fmax, spec)
    valid = ~np.isnan(rep)
    pcts  = np.linspace(0, 100, KEYS + 1)[1:-1]
    if valid.sum() < KEYS:                         # 노트가 너무 적으면 로그 등분
        inner = np.geomspace(max(fmin, 1.0), fmax, KEYS + 1)[1:-1]
    elif bounds == "gap":
        inner = np.array(gap_bounds(rep[valid], KEYS, balance))
    else:
        inner = np.percentile(rep[valid], pcts)

    M = np.zeros((len(onset_frames), KEYS), dtype=bool)
    lanes = np.searchsorted(inner, rep[valid], side="right")   # 0 ~ KEYS-1
    M[np.flatnonzero(valid), lanes] = True
    info = {
        "q_fmin": fmin, "q_fmax": fmax, "q_spec": spec, "q_bounds": bounds, "q_balance": balance,
        "q_bounds_hz": [round(float(v), 1) for v in [fmin, *inner, fmax]],
    }
    return M, rep, info


# ═══════════════════════════════════════════════════════════
# (2) 멜 필터 방식 — 키 하나당 멜 필터 하나
# ═══════════════════════════════════════════════════════════

def assign_mel(S, sr, onset_frames, fmin, fmax, threshold=2.0,
               freqs=None, auto_range=True, pct=5.0, spec="flux", centers="quantile"):
    """
    멜 필터 KEYS개(삼각형)를 만들고, onset의 파워 스펙트럼
    (spec: flux = 어택에서 늘어난 파워 / frame = 프레임 파워)에 곱해 필터별 에너지를 구한다.
      · auto_range=True(기본)면 필터를 곡에 맞춘다: onset 대표 주파수 분포의
        pct ~ (100-pct) 퍼센타일에 첫 필터 / 마지막 필터의 꼭짓점이 온다.
        그 사이 꼭짓점은 centers="quantile"(기본)이면 분포의 퍼센타일, "even"이면 멜 등간격.
        (auto_mel_centers + tri_filterbank)
        False면 fmin~fmax를 멜 등분한 librosa 필터 그대로.
      · 레인마다 따로: 필터 에너지 ÷ 그 필터의 평소값(곡 전체 onset 중앙값) ≥ threshold 이면
        그 레인에 노트. 여러 개 넘으면 동시치기.
        하나도 안 넘으면 평소 대비 가장 큰 필터 하나에 노트 → 노트 시각 수는 (1) 방식과 같다.
    반환: (레인 행렬[onset, KEYS], 필터 에너지 행렬[onset, KEYS], 필터 뱅크, info)
    """
    if freqs is None:
        freqs = librosa.fft_frequencies(sr=sr, n_fft=N_FFT)
    if auto_range:
        rep  = onset_rep_freq(S, freqs, onset_frames, 20.0, sr / 2, spec)
        peak = auto_mel_centers(rep, KEYS, pct, centers)
        fb   = tri_filterbank(freqs, peak)
        nz   = np.flatnonzero(fb.sum(axis=0) > 0)
        fmin, fmax = float(freqs[nz[0]]), float(freqs[nz[-1]])   # 실제로 덮는 범위
    else:
        fb   = librosa.filters.mel(sr=sr, n_fft=N_FFT, n_mels=KEYS, fmin=fmin, fmax=fmax)
        peak = librosa.mel_frequencies(n_mels=KEYS + 2, fmin=fmin, fmax=fmax)[1:-1]
    E = (fb @ onset_power(S, onset_frames, spec)).T     # [onset, KEYS]

    ref = np.median(E, axis=0) if len(E) else np.ones(KEYS)
    ref[ref <= 0] = 1.0
    ratio = E / ref                                     # 평소 대비 몇 배 울렸나
    M = ratio >= threshold                              # 레인마다 따로 판단
    empty = ~M.any(axis=1)                              # 아무 필터도 안 넘은 onset은
    M[empty, np.argmax(ratio[empty], axis=1)] = True    # 평소 대비 가장 큰 필터 하나에 노트 (시각 수 유지)

    info = {
        "mel_fmin": round(float(fmin), 1), "mel_fmax": round(float(fmax), 1),
        "mel_auto_range": auto_range, "mel_pct": pct, "mel_spec": spec,
        "mel_centers": centers if auto_range else "fixed", "mel_threshold": threshold,
        "filter_centers_hz": [round(float(c), 1) for c in peak],      # 필터별 꼭짓점
    }
    return M, E, fb, info


# ═══════════════════════════════════════════════════════════
# 터미널 비교 출력
# ═══════════════════════════════════════════════════════════

def thin_times(times, frames, gap):
    """
    레인 배치 전, 노트 시각을 최소 간격 gap초로 솎는다 (키와 무관, 두 방식 공통).
    직전에 남긴 시각과 gap초보다 가까운 시각은 버린다. 반환: (시각, 프레임)
    """
    times, frames = np.asarray(times, dtype=float), np.asarray(frames)
    if gap <= 0 or times.size == 0:
        return times, frames
    keep, last = [], -np.inf
    for i in np.argsort(times, kind="stable"):
        if times[i] - last >= gap:
            keep.append(i)
            last = times[i]
    return times[keep], frames[keep]


def lane_stats(M):
    """
    M: 레인 행렬[onset, KEYS] (True = 그 레인을 침)
    반환: (레인별 노트 수, 동시치기 수, 같은 레인 연속 횟수, 한 레인 최대 연속)
    """
    M = M[M.any(axis=1)]
    counts = [int(c) for c in M.sum(axis=0)]
    chords = int((M.sum(axis=1) >= 2).sum())
    same = int((M[1:] & M[:-1]).any(axis=1).sum()) if len(M) > 1 else 0
    max_run = 0
    for k in range(KEYS):
        run = 0
        for v in M[:, k]:
            run = run + 1 if v else 0
            max_run = max(max_run, run)
    return counts, chords, same, max_run


def print_comparison(times, M1, M2, max_rows):
    half = KEYS // 2

    def row(c):
        return " ".join(c[:half]) + " | " + " ".join(c[half:])

    def cells(m):
        return row(["■" if v else "·" for v in m])

    head  = row([str(k) for k in range(1, KEYS + 1)])
    width = len(head)
    print("\n── 레인 비교 ────────────────────────────────────────────────")
    print(f"   시간(초)   {'(1) STFT':<{width}}     {'(2) ' + m2_name():<{width - 2}}     일치")
    print(f"              {head}     {head}")
    print("  " + "-" * (24 + width * 2))
    for t, a, b in list(zip(times, M1, M2))[:max_rows]:
        mark = "O" if (a == b).all() else ""
        if b.sum() >= 2:
            mark += " 동시"
        print(f"  {t:8.3f}   {cells(a)}     {cells(b)}     {mark}")
    if len(times) > max_rows:
        print(f"  ... 외 {len(times) - max_rows}개 (--rows 로 더 보기)")

    valid = M1.any(axis=1) & M2.any(axis=1)
    agree = float((M1[valid] == M2[valid]).all(axis=1).mean()) if valid.any() else 0.0
    # 왼손/오른손 그룹 일치: 두 방식이 쓴 손(왼쪽 절반/오른쪽 절반)이 같은 비율
    hands = lambda M: np.stack([M[:, :half].any(axis=1), M[:, half:].any(axis=1)], axis=1)
    side  = float((hands(M1)[valid] == hands(M2)[valid]).all(axis=1).mean()) if valid.any() else 0.0
    print("\n── 요약 ─────────────────────────────────────────────────────")
    print(f"  노트 시각 {int(valid.sum())}개 | 레인 일치 {agree:.0%} | 왼손/오른손 그룹 일치 {side:.0%}")
    names = [f"레인{k}" for k in range(1, KEYS + 1)]
    print(f"              {'  '.join(names[:half])} | {'  '.join(names[half:])}   "
          f"동시치기  같은 레인 연속  한 레인 최대 연속")
    for name, M in (("(1) STFT  ", M1), (f"(2) {m2_name()}", M2)):
        counts, chords, same, max_run = lane_stats(M)
        c = [f"{x:5d}" for x in counts]
        print(f"  {name}  {'  '.join(c[:half])} | {'  '.join(c[half:])}   "
              f"{chords:6d}회  {same:12d}회  {max_run:14d}")
    return agree, side


# ═══════════════════════════════════════════════════════════
# 비교 그림 (--png, matplotlib이 있을 때만)
# ═══════════════════════════════════════════════════════════

def plot_comparison(path, title, y, sr, S, times, M1, M2, stft_info, fb, mel_info, agree):
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        from matplotlib import font_manager
        from matplotlib.patches import Patch
    except ImportError:
        print("  [그림 생략] matplotlib이 없습니다. pip install matplotlib 로 설치해 주세요.")
        return

    installed = {f.name for f in font_manager.fontManager.ttflist}
    font = next((n for n in ("Malgun Gothic", "AppleGothic", "NanumGothic") if n in installed), "DejaVu Sans")
    plt.rcParams.update({"font.family": font, "axes.unicode_minus": False, "font.size": 10})
    INK, MUTED, GRID, SURF = "#0b0b0b", "#52514e", "#d8d7d0", "#fcfcfb"
    dur   = len(y) / sr
    freqs = librosa.fft_frequencies(sr=sr, n_fft=N_FFT)

    fig = plt.figure(figsize=(17, 11), facecolor=SURF)
    gs  = fig.add_gridspec(4, 2, width_ratios=[4.2, 1], height_ratios=[1.6, 1, 1, 0.9],
                           hspace=0.55, wspace=0.08)

    def style(ax):
        ax.set_facecolor(SURF)
        for sp in ("top", "right"):
            ax.spines[sp].set_visible(False)
        for sp in ("left", "bottom"):
            ax.spines[sp].set_color(GRID)
        ax.tick_params(colors=MUTED)

    def heading(ax, text, sub=""):
        ax.set_title(text, loc="left", fontsize=12, fontweight="bold", color=INK, pad=16 if sub else 6)
        if sub:
            ax.text(0, 1.02, sub, transform=ax.transAxes, fontsize=9, color=MUTED, va="bottom")

    # ── 스펙트로그램 + onset ─────────────────────────────
    ax0 = fig.add_subplot(gs[0, 0]); style(ax0)
    mel = librosa.feature.melspectrogram(S=S ** 2, sr=sr, n_mels=96, fmax=8000)
    ax0.imshow(librosa.power_to_db(mel, ref=np.max), origin="lower", aspect="auto", cmap="Greys",
               vmin=-70, vmax=0, extent=[0, dur, 0, 96])
    mel_f = librosa.mel_frequencies(n_mels=96, fmax=8000)
    ticks = [100, 300, 1000, 3000, 8000]
    ax0.set_yticks([np.searchsorted(mel_f, f) for f in ticks])
    ax0.set_yticklabels([f"{f}Hz" if f < 1000 else f"{f // 1000}kHz" for f in ticks])
    ax0.vlines(times, 93, 96, color=INK, lw=0.6)
    heading(ax0, f"{title} — 스펙트로그램과 onset",
            f"위쪽 눈금 = 감지된 onset {len(times)}개 (두 방식 공통). 레인은 이 순간의 스펙트럼으로 정한다.")

    # ── 레인 피아노롤 (1) / (2) ──────────────────────────
    rows = [(gs[1, 0], M1, f"(1) STFT 방식 — {stft_label()}"),
            (gs[2, 0], M2, f"(2) {m2_name()} 방식 — {m2_label()}"
                           + (f" (threshold: 평소의 {mel_info['mel_threshold']:g}배)" if METHOD2 == "mel" else
                              f" (기준: {mel_info['q_spec']})"))]
    for i, (g, M, label) in enumerate(rows):
        ax = fig.add_subplot(g, sharex=ax0); style(ax)
        ni, li = np.nonzero(M)
        chord = M.sum(axis=1) >= 2
        if chord.any():                                  # 동시치기는 세로선으로 이음
            first = np.argmax(M[chord], axis=1) + 1
            last  = KEYS - np.argmax(M[chord][:, ::-1], axis=1)
            ax.vlines(times[chord], first, last, color=MUTED, lw=1, zorder=2)
        ax.scatter(times[ni], li + 1, c=[LANE_COLORS[l] for l in li],
                   s=22, edgecolor=SURF, linewidth=0.8, zorder=3)
        ax.axhline(KEYS // 2 + 0.5, color=GRID, lw=1)
        ax.set_ylim(KEYS + 0.6, 0.4); ax.set_yticks(range(1, KEYS + 1))
        ax.set_yticklabels([f"레인{k}" for k in range(1, KEYS + 1)])
        ax.grid(axis="y", color=GRID, lw=0.5, alpha=0.6)
        sub = f"레인 일치율 {agree:.0%} (두 방식이 같은 레인을 고른 비율)" if i == 1 else ""
        heading(ax, label, sub)
        if i == 1:
            ax.set_xlabel("시간 (초)", color=MUTED)
    ax0.set_xlim(0, dur)

    # ── 오른쪽: 각 방식의 주파수 대역 ───────────────────
    axb = fig.add_subplot(gs[1, 1]); style(axb)
    edges = stft_info["low_bounds"] + stft_info["high_bounds"][1:]
    for k in range(KEYS):
        lo, hi = max(edges[k], 20.0), edges[k + 1]
        axb.axvspan(lo, hi, color=LANE_COLORS[k], alpha=0.75, lw=0)
        axb.text(np.sqrt(lo * hi), 0.5, str(k + 1), ha="center", va="center", fontsize=9,
                 color=INK, fontweight="bold")
    axb.axvline(stft_info["low_high_boundary"], color=INK, lw=1.2)
    axb.set_xscale("log"); axb.set_xlim(20, sr / 2); axb.set_yticks([])
    heading(axb, "(1) 레인별 대역", f"검은 선 = 저/고음 경계 {stft_info['low_high_boundary']:.0f}Hz")

    axm = fig.add_subplot(gs[2, 1]); style(axm)
    if METHOD2 == "quantile":
        qb = mel_info["q_bounds_hz"]
        for k in range(KEYS):
            lo, hi = max(qb[k], 20.0), max(qb[k + 1], 20.0)
            axm.axvspan(lo, hi, color=LANE_COLORS[k], alpha=0.75, lw=0)
            axm.text(np.sqrt(lo * hi), 0.5, str(k + 1), ha="center", va="center", fontsize=9,
                     color=INK, fontweight="bold")
        axm.set_xscale("log"); axm.set_xlim(20, sr / 2); axm.set_yticks([])
        axm.set_xlabel("주파수 (Hz)", color=MUTED)
        heading(axm, "(2) 레인별 대역", f"onset 대표 주파수 {KEYS}등분 ({mel_info['q_spec']})")
    else:
        for k in range(KEYS):
            w = fb[k] / fb[k].max()
            axm.fill_between(freqs, w, color=LANE_COLORS[k], alpha=0.55, lw=0)
            axm.plot(freqs, w, color=LANE_COLORS[k], lw=1.2)
            c = mel_info["filter_centers_hz"][k]
            axm.text(c, 1.05, str(k + 1), ha="center", fontsize=9, color=INK, fontweight="bold")
        axm.set_xscale("log"); axm.set_xlim(20, sr / 2); axm.set_ylim(0, 1.2); axm.set_yticks([])
        axm.set_xlabel("주파수 (Hz)", color=MUTED)
        heading(axm, f"(2) 멜 필터 {KEYS}개", f"{mel_info['mel_fmin']:g}~{mel_info['mel_fmax']:g}Hz")

    # ── 아래: 레인별 노트 수 ─────────────────────────────
    axc = fig.add_subplot(gs[3, :]); style(axc)
    c1 = lane_stats(M1)[0]
    c2 = lane_stats(M2)[0]
    x = np.arange(1, KEYS + 1)
    for off, counts, hatch in ((-0.2, c1, None), (0.2, c2, "///")):
        bars = axc.bar(x + off, counts, width=0.38, color=LANE_COLORS[:KEYS], edgecolor=SURF,
                       linewidth=2, hatch=hatch)
        for b, v in zip(bars, counts):
            axc.text(b.get_x() + b.get_width() / 2, v, str(v), ha="center", va="bottom",
                     fontsize=8.5, color=MUTED)
    axc.set_xticks(x); axc.set_xticklabels([f"레인{k}" for k in x])
    axc.set_ylabel("노트 수", color=MUTED)
    axc.legend(handles=[Patch(facecolor="#b5b4ad", label="(1) STFT"),
                        Patch(facecolor="#b5b4ad", hatch="///", label=f"(2) {m2_name()}")],
               frameon=False, loc="upper right")
    heading(axc, "레인별 노트 수 (왼쪽 막대 = (1), 빗금 막대 = (2))")

    fig.savefig(path, dpi=110, bbox_inches="tight", facecolor=SURF)
    plt.close(fig)
    print(f"  그림 저장: {path}")


# ═══════════════════════════════════════════════════════════
# 비교 재생 (pygame)
# ═══════════════════════════════════════════════════════════

W, H        = 1280, 720
FPS         = 60
JUDGE_Y     = H - 130
TOP_Y       = 70
MARGIN      = 40
GROUP_GAP   = 16
FLASH_MS    = 120
CLAP_PATH   = "Clap.mp3"   # 있으면 노트가 판정선에 닿을 때 재생 (chart_visualizer.py와 같음)

BG, LANE_BG, LANE_BORDER = (18, 18, 26), (28, 28, 40), (50, 50, 70)
JUDGE_LINE, DIVIDER      = (220, 220, 255), (80, 80, 120)
TEXT, MUTED_TXT          = (210, 210, 230), (120, 120, 150)
COOLS = [(80, 140, 255), (110, 180, 255), (150, 215, 255)]       # 왼손(저음) 쿨톤
WARMS = [(255, 180, 80), (255, 130, 110), (255, 90, 160)]        # 오른손(고음) 웜톤


def lane_palette():
    """레인별 색: 왼쪽 절반 쿨톤, 오른쪽 절반 웜톤 (chart_visualizer.py와 같은 규칙)."""
    half = KEYS // 2
    return COOLS[:half] + WARMS[:KEYS - half]


def fmt_time(sec):
    m, s = divmod(int(max(0.0, sec)), 60)
    return f"{m}:{s:02d}"


class ComparePlayer:
    """같은 onset 시각에 두 방식의 레인을 좌우 화면에 나란히 떨어뜨린다."""

    def __init__(self, audio_path, times, M1, M2, duration):
        self.audio_path = audio_path
        self.times      = np.asarray(times, dtype=float)
        self.lanes      = [np.asarray(M1, dtype=bool), np.asarray(M2, dtype=bool)]   # [onset, KEYS]
        self.differ     = (self.lanes[0] != self.lanes[1]).any(axis=1)
        self.counts     = [[int(c) for c in M.sum(axis=0)] for M in self.lanes]
        self.duration   = duration
        self.speed      = 320.0                        # 노트 낙하 속도(px/초)
        self.flash      = np.zeros((2, KEYS))
        self.rgb        = lane_palette()
        self.paused     = False
        self.song_time  = 0.0
        self.last_time  = 0.0

        valid = self.lanes[0].any(axis=1) & self.lanes[1].any(axis=1)
        self.agree = float(np.mean(~self.differ[valid])) if valid.any() else 0.0

        # 패널 2개 × 레인 6개의 중심 x 좌표
        panel_w     = (W - MARGIN * 3) / 2
        self.lane_w = (panel_w - GROUP_GAP) / KEYS
        self.note_w = int(self.lane_w * 0.82)
        self.note_h = 18
        self.panels = []
        for p in range(2):
            x0 = MARGIN + p * (panel_w + MARGIN)
            xs = [int(x0 + (k + 0.5) * self.lane_w + (GROUP_GAP if k >= KEYS // 2 else 0))
                  for k in range(KEYS)]
            self.panels.append((x0, panel_w, xs))

    # ── 시간 ─────────────────────────────────────────────
    def now(self):
        if self.paused:
            return self.song_time
        return self.time_ref + (pygame.time.get_ticks() - self.wall_ref) / 1000.0

    def play_from(self, t):
        self.song_time = self.last_time = max(0.0, min(t, self.duration))
        self.flash[:] = 0
        if self.have_music and not self.paused:
            pygame.mixer.music.play(start=self.song_time)
        self.wall_ref = pygame.time.get_ticks()
        self.time_ref = self.song_time

    def toggle_pause(self):
        if self.paused:
            self.paused = False
            if self.have_music:
                pygame.mixer.music.unpause()
            self.wall_ref = pygame.time.get_ticks()
            self.time_ref = self.song_time
        else:
            self.song_time = self.now()
            self.paused = True
            if self.have_music:
                pygame.mixer.music.pause()

    # ── 판정선 통과 처리 ─────────────────────────────────
    def update(self, t, dt):
        i0 = np.searchsorted(self.times, self.last_time, side="right")
        i1 = np.searchsorted(self.times, t, side="right")
        for i in range(i0, i1):
            for p in range(2):
                self.flash[p, self.lanes[p][i]] = FLASH_MS
        hit = any(self.lanes[p][i].any() for p in range(2) for i in range(i0, i1))
        if hit and self.clap is not None:
            self.clap.play()
        self.last_time = t
        self.flash = np.maximum(0, self.flash - dt * 1000)

    # ── 그리기 ───────────────────────────────────────────
    def draw(self, t):
        scr = self.screen
        scr.fill(BG)
        look = (JUDGE_Y - TOP_Y) / self.speed
        lo = np.searchsorted(self.times, t - 0.05)
        hi = np.searchsorted(self.times, t + look)
        titles = [f"(1) STFT — {stft_label()}", f"(2) {m2_name()} — {m2_label()}"]

        for p, (x0, pw, xs) in enumerate(self.panels):
            for cx in xs:
                rect = pygame.Rect(int(cx - self.lane_w / 2 + 2), TOP_Y,
                                   int(self.lane_w - 4), JUDGE_Y - TOP_Y + 40)
                pygame.draw.rect(scr, LANE_BG, rect)
                pygame.draw.rect(scr, LANE_BORDER, rect, 1)
            mid = (xs[KEYS // 2 - 1] + xs[KEYS // 2]) // 2
            pygame.draw.line(scr, DIVIDER, (mid, TOP_Y), (mid, JUDGE_Y + 40), 2)

            # 노트 (판정선까지 남은 시간 × 속도 = 판정선 위 거리)
            for i in range(lo, hi):
                ks = np.flatnonzero(self.lanes[p][i])
                if ks.size == 0:
                    continue
                y = int(JUDGE_Y - (self.times[i] - t) * self.speed)
                if ks.size >= 2:                        # 동시치기: 노트끼리 가로선으로 이음
                    pygame.draw.line(scr, (235, 235, 255), (xs[ks[0]], y), (xs[ks[-1]], y), 3)
                for k in ks:
                    rect = pygame.Rect(xs[k] - self.note_w // 2, y - self.note_h // 2,
                                       self.note_w, self.note_h)
                    pygame.draw.rect(scr, self.rgb[k], rect, border_radius=5)
                    if self.differ[i]:
                        pygame.draw.rect(scr, (255, 255, 255), rect.inflate(4, 4), 2, border_radius=6)

            # 판정선 + 버튼 플래시 + 레인 번호/노트 수
            pygame.draw.line(scr, JUDGE_LINE, (int(x0) - 6, JUDGE_Y), (int(x0 + pw) + 6, JUDGE_Y), 2)
            for k, cx in enumerate(xs):
                btn = pygame.Rect(cx - self.note_w // 2, JUDGE_Y - self.note_h // 2, self.note_w, self.note_h)
                pygame.draw.rect(scr, tuple(int(c * 0.35) for c in self.rgb[k]), btn, border_radius=5)
                a = min(1.0, self.flash[p, k] / FLASH_MS)
                if a > 0:
                    fs = pygame.Surface(btn.size, pygame.SRCALPHA)
                    pygame.draw.rect(fs, (*self.rgb[k], int(220 * a)), fs.get_rect(), border_radius=5)
                    scr.blit(fs, btn.topleft)
                lbl = self.font_md.render(str(k + 1), True, self.rgb[k])
                scr.blit(lbl, lbl.get_rect(centerx=cx, centery=JUDGE_Y + 26))
                cnt = self.font_sm.render(f"{self.counts[p][k]}개", True, MUTED_TXT)
                scr.blit(cnt, cnt.get_rect(centerx=cx, centery=JUDGE_Y + 46))

            scr.blit(self.font_lg.render(titles[p], True, TEXT), (int(x0), 16))
            low = self.font_sm.render("◀ 저음", True, self.rgb[0])
            high = self.font_sm.render("고음 ▶", True, self.rgb[-1])
            scr.blit(low, (int(x0), 46))
            scr.blit(high, (int(x0 + pw) - high.get_width(), 46))

        # 하단 HUD
        bar = pygame.Rect(MARGIN, H - 46, W - MARGIN * 2, 8)
        pygame.draw.rect(scr, (50, 50, 70), bar, border_radius=4)
        if self.duration > 0:
            fill = bar.copy()
            fill.width = int(bar.width * min(1.0, t / self.duration))
            pygame.draw.rect(scr, (120, 160, 255), fill, border_radius=4)
        passed = int(np.searchsorted(self.times, t, side="right"))
        info = (f"{fmt_time(t)} / {fmt_time(self.duration)}     노트 {passed} / {len(self.times)}     "
                f"레인 일치 {self.agree:.0%}     흰 테두리 = 두 방식이 다름     가로선 = 동시치기     "
                f"속도 {self.speed:.0f}px/s")
        scr.blit(self.font_sm.render(info, True, TEXT), (MARGIN, H - 72))
        hint = self.font_sm.render("SPACE 일시정지  |  ← → 5초 이동  |  ↑ ↓ 노트 속도  |  Q 종료",
                                   True, MUTED_TXT)
        scr.blit(hint, hint.get_rect(centerx=W // 2, centery=H - 22))
        if self.paused:
            pt = self.font_xl.render("PAUSED", True, (255, 200, 80))
            scr.blit(pt, pt.get_rect(center=(W // 2, H // 2)))

    # ── 메인 루프 ────────────────────────────────────────
    def run(self, max_frames=None, screenshot=None):
        pygame.init()
        pygame.mixer.init(frequency=44100, size=-16, channels=2, buffer=512)
        self.screen = pygame.display.set_mode((W, H))
        pygame.display.set_caption(f"Lane Compare {KEYS}key — (1) STFT vs (2) "
                                   + ("Quantile" if METHOD2 == "quantile" else "Mel filter"))
        self.clock = pygame.time.Clock()
        fonts = "malgungothic,applegothic,nanumgothic,arial"
        self.font_sm = pygame.font.SysFont(fonts, 14)
        self.font_md = pygame.font.SysFont(fonts, 17, bold=True)
        self.font_lg = pygame.font.SysFont(fonts, 19, bold=True)
        self.font_xl = pygame.font.SysFont(fonts, 32, bold=True)

        try:
            pygame.mixer.music.load(self.audio_path)
            self.have_music = True
        except Exception as e:
            print(f"[경고] 오디오 재생 불가: {e} — 소리 없이 진행합니다.")
            self.have_music = False

        self.clap = None
        for cand in (CLAP_PATH, os.path.join(os.path.dirname(os.path.abspath(self.audio_path)), CLAP_PATH)):
            if os.path.exists(cand):
                try:
                    self.clap = pygame.mixer.Sound(cand)
                    self.clap.set_volume(0.8)
                    pygame.mixer.set_num_channels(16)
                except Exception:
                    self.clap = None
                break

        self.play_from(0.0)
        frames = 0
        while True:
            dt = self.clock.tick(FPS) / 1000.0
            for ev in pygame.event.get():
                if ev.type == pygame.QUIT or (ev.type == pygame.KEYDOWN
                                              and ev.key in (pygame.K_q, pygame.K_ESCAPE)):
                    pygame.quit()
                    return
                if ev.type == pygame.KEYDOWN:
                    if ev.key == pygame.K_SPACE:
                        self.toggle_pause()
                    elif ev.key in (pygame.K_LEFT, pygame.K_RIGHT):
                        self.play_from(self.now() + (-5.0 if ev.key == pygame.K_LEFT else 5.0))
                    elif ev.key == pygame.K_UP:
                        self.speed = min(900.0, self.speed + 40)
                    elif ev.key == pygame.K_DOWN:
                        self.speed = max(120.0, self.speed - 40)

            t = self.now()
            if not self.paused:
                self.song_time = t
                self.update(t, dt)
            self.draw(t)
            pygame.display.flip()

            frames += 1
            if max_frames and frames >= max_frames:      # 테스트용: 정해진 프레임 후 화면 저장 후 종료
                if screenshot:
                    pygame.image.save(self.screen, screenshot)
                pygame.quit()
                return
            if not self.paused and t >= self.duration + 1.0:
                pygame.quit()
                return


# ═══════════════════════════════════════════════════════════
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="레인 배치 비교 플레이어 (STFT vs 멜 필터)")
    parser.add_argument("input", help="오디오 파일, 또는 채보 JSON (그 채보의 노트 시각을 그대로 사용)")
    parser.add_argument("--audio", default=None, help="채보 JSON을 줄 때 오디오 경로 직접 지정 (기본: 채보의 source)")
    parser.add_argument("--keys", type=int, choices=[2, 4, 6], default=KEYS,
                        help=f"키 수 (기본 {KEYS})")
    parser.add_argument("--onset", choices=list(ONSET_PRESETS), default="easy",
                        help="onset 감지 방식. easy = chart_generator.py easy와 동일(기본), "
                             "6k-easy|6k-medium|6k-hard = chart_generator.py 6key 감도")
    parser.add_argument("--method2", choices=["quantile", "mel"], default=METHOD2,
                        help="오른쪽 (2) 방식. quantile: 대표 주파수 퍼센타일 등분(기본), mel: 멜 필터")
    parser.add_argument("--q-spec", choices=["flux", "frame"], default="flux",
                        help="(2) quantile 대표 주파수 기준. flux: 어택에서 늘어난 파워(기본), "
                             "frame: onset 프레임 파워 그대로")
    parser.add_argument("--q-bounds", choices=["gap", "exact"], default="gap",
                        help="(2) quantile 경계. gap: 균등에 가깝되 소리 무리 사이 빈틈에 둠(기본), "
                             "exact: 퍼센타일 정확히 등분")
    parser.add_argument("--q-balance", type=float, default=1.0,
                        help="(2) gap 경계에서 노트 수 균등을 얼마나 중시할지. 클수록 exact에 가까움, "
                             "0이면 빈틈만 봄. 기본 1.0")
    parser.add_argument("--q-fmin", type=float, default=40.0, help="(2) quantile 사용 최저 주파수(Hz). 기본 40")
    parser.add_argument("--q-fmax", type=float, default=8000.0, help="(2) quantile 사용 최고 주파수(Hz). 기본 8000")
    parser.add_argument("--mel-range", choices=["auto", "fixed"], default="auto",
                        help="멜 필터 범위. auto: onset 대표 주파수 분포에 맞춤(기본), "
                             "fixed: --mel-fmin ~ --mel-fmax 그대로")
    parser.add_argument("--mel-pct", type=float, default=5.0,
                        help="auto 범위: 대표 주파수의 이 퍼센타일 ~ (100-이 값) 퍼센타일에 "
                             "양 끝 필터의 꼭짓점을 둠. 0이면 최소~최대. 기본 5")
    parser.add_argument("--mel-centers", choices=["quantile", "even"], default="quantile",
                        help="auto 범위에서 필터 꼭짓점 배치. quantile: 대표 주파수 분포의 퍼센타일(기본), "
                             "even: 양 끝 사이 멜 등간격")
    parser.add_argument("--mel-spec", choices=["flux", "frame"], default="flux",
                        help="멜 필터에 넣을 스펙트럼. flux: 어택에서 늘어난 파워(기본), frame: 프레임 파워 그대로")
    parser.add_argument("--mel-fmin", type=float, default=40.0, help="fixed 범위 최저 주파수(Hz). 기본 40")
    parser.add_argument("--mel-fmax", type=float, default=8000.0, help="fixed 범위 최고 주파수(Hz). 기본 8000")
    parser.add_argument("--mel-threshold", type=float, default=2.0,
                        help="(2) 레인마다: 필터 에너지가 평소(곡 전체 onset 중앙값)의 몇 배 이상이면 "
                             "그 레인에 노트를 만들지. 클수록 노트가 적음. 기본 2.0")
    parser.add_argument("--min-gap", type=float, default=0.25,
                        help="노트 최소 간격(초). 레인 배치 전에 onset 시각을 이 간격으로 솎는다 "
                             "(키와 무관, 두 방식 공통). 0이면 끔. 기본 0.25")
    parser.add_argument("--rows", type=int, default=20, help="터미널에 나란히 보여 줄 노트 수. 기본 20")
    parser.add_argument("--png", action="store_true",
                        help="비교 그림(<곡>_lanes_compare.png)을 음원 폴더에 저장 (matplotlib 필요)")
    parser.add_argument("--no-play", action="store_true", help="재생 창 없이 터미널 비교만")
    args = parser.parse_args()
    KEYS = args.keys
    METHOD2 = args.method2

    chart_times = None
    if args.input.lower().endswith(".json"):
        if not os.path.exists(args.input):
            sys.exit(f"채보 파일 없음: {args.input}")
        chart_times, chart_audio, chart_diff = load_chart_times(args.input)
        args.audio = args.audio or chart_audio
        print(f"[채보] {args.input} ({chart_diff}) — 노트 시각 {len(chart_times)}개를 그대로 사용")
    else:
        args.audio = args.input
    if not os.path.exists(args.audio):
        sys.exit(f"오디오 파일 없음: {args.audio}")
    if not args.no_play:
        try:
            import pygame
        except ImportError:
            sys.exit("pygame이 없습니다. pip install pygame 으로 설치하거나 --no-play 로 실행하세요.")

    print(f"[1/3] 오디오 로딩: {args.audio}")
    y, sr = librosa.load(args.audio, sr=None, mono=True)
    duration = len(y) / sr
    args.mel_fmax = min(args.mel_fmax, sr / 2)
    args.q_fmax   = min(args.q_fmax, sr / 2)

    if chart_times is not None:
        print("[2/3] 노트 시각: 채보 JSON 사용 (onset 감지 생략)")
        onset_times  = chart_times
        onset_frames = librosa.time_to_frames(onset_times, sr=sr, hop_length=HOP)
    else:
        print(f"[2/3] Onset 감지 ({args.onset})...")
        onset_frames, onset_times = detect_onsets(y, sr, args.onset)
        print(f"  ↳ onset {len(onset_times)}개")
    if args.min_gap > 0:
        n0 = len(onset_times)
        onset_times, onset_frames = thin_times(onset_times, onset_frames, args.min_gap)
        print(f"  ↳ 최소 간격 {args.min_gap:g}초로 솎음: {n0}개 → {len(onset_times)}개 (두 방식 공통)")

    print(f"[3/3] 레인 배치 (1) STFT / (2) {m2_name()}...")
    S     = np.abs(librosa.stft(y, n_fft=N_FFT, hop_length=HOP))
    freqs = librosa.fft_frequencies(sr=sr, n_fft=N_FFT)
    l1, stft_info = assign_stft(S, freqs, sr, onset_frames)
    inner = lambda b: "/".join(f"{v:.0f}" for v in b[1:-1]) + "Hz" if len(b) > 2 else "없음"
    print(f"  ↳ (1) 저/고음 경계 {stft_info['low_high_boundary']:.0f}Hz, "
          f"그룹 안 경계: 저음 {inner(stft_info['low_bounds'])}, 고음 {inner(stft_info['high_bounds'])}")
    if METHOD2 == "quantile":
        fb = None
        l2, rep, mel_info = assign_quantile(S, freqs, onset_frames, args.q_fmin, args.q_fmax,
                                            args.q_spec, args.q_bounds, args.q_balance)
        print(f"  ↳ (2) 대표 주파수({args.q_spec}, 경계 {args.q_bounds}) 레인 경계: "
              f"{inner(mel_info['q_bounds_hz'])} "
              f"(사용 범위 {args.q_fmin:g}~{args.q_fmax:g}Hz)")
    else:
        l2, _, fb, mel_info = assign_mel(S, sr, onset_frames, args.mel_fmin, args.mel_fmax,
                                         threshold=args.mel_threshold, freqs=freqs,
                                         auto_range=args.mel_range == "auto", pct=args.mel_pct,
                                         spec=args.mel_spec, centers=args.mel_centers)
        rng = (f"auto, 꼭짓점 = 대표 주파수 {args.mel_pct:g}~{100 - args.mel_pct:g}% 범위의 "
               + ("퍼센타일" if args.mel_centers == "quantile" else "멜 등간격")
               if args.mel_range == "auto" else "fixed")
        print(f"  ↳ (2) 멜 필터 범위 {mel_info['mel_fmin']:.0f}~{mel_info['mel_fmax']:.0f}Hz ({rng}), "
              f"스펙트럼 {args.mel_spec}")
        print(f"  ↳ (2) 멜 필터 꼭짓점 {', '.join(f'{c:.0f}' for c in mel_info['filter_centers_hz'])}Hz")
        print(f"  ↳ (2) threshold 평소의 {args.mel_threshold:g}배: 동시치기 {int((l2.sum(axis=1) >= 2).sum())}개")


    agree, _ = print_comparison(onset_times, l1, l2, args.rows)

    if args.png:
        base = os.path.splitext(os.path.abspath(args.audio))[0]
        plot_comparison(base + "_lanes_compare.png", os.path.basename(args.audio), y, sr, S,
                        onset_times, l1, l2, stft_info, fb, mel_info, agree)

    if not args.no_play:
        print(f"\n비교 재생 시작 — 왼쪽 (1) STFT, 오른쪽 (2) {m2_name()}")
        ComparePlayer(args.audio, onset_times, l1, l2, duration).run()