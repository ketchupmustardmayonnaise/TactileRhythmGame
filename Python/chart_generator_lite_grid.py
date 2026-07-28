"""
2key / 4key 전용 '쉬움' 채보 생성기  (비트 격자 스냅 적용판)
──────────────────────────────────────────────────────────
기존 chart_generator.py(6key)는 그대로 두고, 더 쉬운 2/4key 채보를 만든다.

이 버전의 변경점(방안 1: 비트 격자 스냅):
  · onset(멜로디 음표 시작)은 노트 소스로 그대로 유지 → 멜로디 기반.
  · 감지된 onset을 곡의 비트/서브비트 격자점으로 '스냅'하여
    실제 음보다 조금 빨리/늦게 잡히는 미세한 어긋남을 교정한다.
  · 격자에서 너무 멀리 떨어진 onset은 (옵션으로) 제외해
    "왜 여기서 노트가 나오지?" 하는 잘못 잡힌 노트를 줄일 수 있다.

제약(요구사항):
  1) 노트와 노트 사이 최소 간격 0.5초  (min_gap)
  2) '동시 치기'(양손 동시)는 2key에만 존재, 4key에는 없음
  3) 같은 키 최대 3연속
  4) 모든 키의 사용 빈도가 비슷하도록 배분

사용법:
    python chart_generator_lite.py <오디오파일> --keys 2 [--difficulty easy|normal]
    python chart_generator_lite.py song.mp3 --keys 4 --output song_4k.json
    # 격자 조정 예: 8분음표 격자 + 격자에서 먼 노트 정리
    python chart_generator_lite.py song.mp3 --keys 4 --subdiv 2 --snap-tolerance 0.3
    # 멜로디(하모닉) 성분만으로 onset 감지
    python chart_generator_lite.py song.mp3 --keys 4 --harmonic

출력 JSON은 게임의 SongLoader가 읽는 형식과 동일하며 lane은 1-based로 저장한다
(2key → lane 1~2, 4key → lane 1~4). 만든 파일은 Assets/Resources/Songs/ 에 넣는다.
"""

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
# min_gap  : 노트 사이 최소 간격(초) — 클수록 쉬움. 하한 0.5.
# approach : 노트가 다가오는 예고 시간(초) — 게임 previewWindow. 클수록 쉬움.
# chord_ratio: 2key 동시치기 비율(0~1). 4key에서는 무시.
DIFFICULTY = {
    "easy":   {"min_gap": 0.70, "approach": 0.80, "chord_ratio": 0.10},
    "normal": {"min_gap": 0.50, "approach": 0.65, "chord_ratio": 0.18},
}

MIN_CHORD_GAP = 1.2   # 동시치기끼리 최소 간격(초): 너무 자주/연속으로 안 나오게
MAX_SAME_RUN  = 3     # 같은 키 최대 연속 횟수


# ═══════════════════════════════════════════════════════════
# 1. 비트 격자 만들기 (beat_track → 서브비트로 세분화)
# ═══════════════════════════════════════════════════════════

def build_grid(beat_times, subdiv, duration):
    """
    beat_track이 찾은 비트 사이를 subdiv등분해 '스냅 격자'를 만든다.
      subdiv=1 → 정박(4분음표) 격자
      subdiv=2 → 8분음표 격자
      subdiv=4 → 16분음표 격자
    곡 시작~끝 전체를 덮도록 비트를 앞뒤로 연장한다.
    비트가 너무 적어 격자를 못 만들면 빈 배열을 반환한다(→ 스냅 생략).
    """
    beat_times = np.asarray(beat_times, dtype=float)
    if beat_times.size < 2:
        return np.array([])

    # 비트 간격(주기)의 중앙값 — 앞뒤 연장에 사용
    period = float(np.median(np.diff(beat_times)))
    if period <= 0:
        return np.array([])

    # 첫 비트 이전 / 마지막 비트 이후로 격자를 연장
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

    full_beats = np.concatenate([np.array(back[::-1]), beat_times, np.array(fwd)])

    # 인접 비트 사이를 subdiv등분
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
# 2. 타이밍 후보 만들기 (onset 감지 → 격자 스냅 → 0.5초 간격으로 솎기)
# ═══════════════════════════════════════════════════════════

def build_times(y, sr, min_gap, grid, snap_tol):
    """onset을 감지 → 비트 격자에 스냅 → min_gap 이상 간격으로 솎아낸다."""
    onset_frames = librosa.onset.onset_detect(y=y, sr=sr, backtrack=True)
    onset_times  = librosa.frames_to_time(onset_frames, sr=sr)

    # ── 방안 1: 비트 격자에 스냅 ─────────────────────────────
    snapped, dropped = snap_to_grid(onset_times, grid, snap_tol)
    snapped = sorted(set(snapped))   # 같은 격자점에 겹친 onset은 하나로 합침
    msg = f"  ↳ onset {len(onset_times)}개 → 스냅 후 {len(snapped)}개"
    if dropped:
        msg += f" (격자에서 먼 {dropped}개 제외)"
    if grid.size == 0:
        msg += "  [경고: 비트를 찾지 못해 스냅 생략]"
    print(msg)

    # ── min_gap 솎기 ────────────────────────────────────────
    kept = []
    last = -1e9
    for t in snapped:
        if t - last >= min_gap:
            kept.append(round(float(t), 4))
            last = t
    return kept


# ═══════════════════════════════════════════════════════════
# 3. 레인 배분 (빈도 균등 + 같은 키 3연속 제한 + 2key 동시치기)
# ═══════════════════════════════════════════════════════════

def assign_lanes(times, keys, chord_ratio, seed=0):
    """
    times   : 정렬된 노트 시각 리스트
    keys    : 2 또는 4
    반환    : [(time, [lane0, lane1, ...]), ...]  (lane은 0-based)

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
# 4. 저장 & 미리보기
# ═══════════════════════════════════════════════════════════

def to_notes(events):
    """(time, [lanes]) → JSON용 노트 리스트. lane을 1-based로 변환."""
    notes = []
    for t, lanes in events:
        for lane in lanes:
            notes.append({"time": t, "lane": lane + 1})
    return notes


def save_chart(notes, audio_path, keys, difficulty, approach, bpm, output_path):
    import os
    data = {
        "source":     os.path.basename(audio_path),   # 오디오는 6key와 공유
        "difficulty": f"{keys}k-{difficulty}",
        "meta": {
            "bpm":              round(float(bpm), 4),
            "seconds_per_beat": round(float(approach), 4),   # 게임 previewWindow로 사용됨
        },
        "notes": notes,
    }
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
    print(f"\n채보 저장 완료: {output_path}  ({len(notes)}노트)")


def print_summary(events, counts, keys):
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
    print(f"  같은 키 최대 연속: {max_run} (제한 {MAX_SAME_RUN})")


# ═══════════════════════════════════════════════════════════
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="2key/4key 쉬움 채보 생성기 (비트 격자 스냅)")
    parser.add_argument("audio")
    parser.add_argument("--keys", type=int, choices=[2, 4], required=True)
    parser.add_argument("--difficulty", choices=["easy", "normal"], default="easy")
    parser.add_argument("--min-gap", type=float, default=None,
                        help="노트 최소 간격(초). 미지정 시 난이도 기본값(하한 0.5)")
    parser.add_argument("--chord-ratio", type=float, default=None,
                        help="2key 동시치기 비율(0~1). 미지정 시 난이도 기본값")
    # ── 방안 1 관련 옵션 ─────────────────────────────────────
    parser.add_argument("--subdiv", type=int, default=4,
                        help="비트 세분화 정도. 1=정박, 2=8분음표, 4=16분음표 격자(기본)")
    parser.add_argument("--snap-tolerance", type=float, default=0.5,
                        help="격자에서 이 비율(격자간격 대비) 이상 떨어진 onset은 제외. "
                             "0.5=모두 유지(순수 스냅), 낮출수록 엄격(예 0.3)")
    parser.add_argument("--harmonic", action="store_true",
                        help="하모닉(멜로디) 성분만으로 onset 감지 — 타악기/저음 노트를 줄임")
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--output", default=None)
    args = parser.parse_args()

    cfg      = DIFFICULTY[args.difficulty]
    min_gap  = max(0.5, args.min_gap if args.min_gap is not None else cfg["min_gap"])
    chord    = args.chord_ratio if args.chord_ratio is not None else cfg["chord_ratio"]
    approach = cfg["approach"]

    print(f"[1/4] 오디오 로딩: {args.audio}")
    y, sr    = librosa.load(args.audio, sr=None, mono=True)
    duration = librosa.get_duration(y=y, sr=sr)

    # 멜로디 강조(선택): 하모닉 성분만 뽑아 타악기/저음 onset을 억제
    analysis = librosa.effects.harmonic(y) if args.harmonic else y

    print("[2/4] 비트 추적 & 격자 생성...")
    tempo, beat_frames = librosa.beat.beat_track(y=y, sr=sr)
    bpm        = float(tempo) if np.isscalar(tempo) else float(tempo[0])
    beat_times = librosa.frames_to_time(beat_frames, sr=sr)
    grid       = build_grid(beat_times, args.subdiv, duration)
    print(f"  ↳ BPM≈{bpm:.1f}, 비트 {len(beat_times)}개, "
          f"격자 {grid.size}점 (subdiv={args.subdiv})")

    print(f"[3/4] 타이밍 후보 생성 (min_gap={min_gap:.2f}s, "
          f"snap_tol={args.snap_tolerance})...")
    times = build_times(analysis, sr, min_gap, grid, args.snap_tolerance)
    print(f"  ↳ 최종 노트 시각 {len(times)}개")

    print(f"[4/4] 레인 배분 ({args.keys}key)...")
    events, counts = assign_lanes(times, args.keys,
                                  chord if args.keys == 2 else 0.0, args.seed)

    notes  = to_notes(events)
    output = args.output or (args.audio.rsplit(".", 1)[0] + f"_{args.keys}k.json")

    print_summary(events, counts, args.keys)
    save_chart(notes, args.audio, args.keys, args.difficulty, approach, bpm, output)