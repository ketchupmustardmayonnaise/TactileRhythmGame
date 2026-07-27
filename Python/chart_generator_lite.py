"""
2key / 4key 전용 '쉬움' 채보 생성기
──────────────────────────────────────────────────────────
기존 chart_generator.py(6key)는 그대로 두고, 더 쉬운 2/4key 채보를 만든다.

제약(요구사항):
  1) 노트와 노트 사이 최소 간격 0.5초  (min_gap)
  2) '동시 치기'(양손 동시)는 2key에만 존재, 4key에는 없음
  3) 같은 키 최대 3연속
  4) 모든 키의 사용 빈도가 비슷하도록 배분

사용법:
    python chart_generator_lite.py <오디오파일> --keys 2 [--difficulty easy|normal]
    python chart_generator_lite.py song.mp3 --keys 4 --output song_4k.json

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
# 1. 타이밍 후보 만들기 (onset 감지 → 0.5초 이상 간격으로 솎기)
# ═══════════════════════════════════════════════════════════

def build_times(y, sr, min_gap):
    """onset을 감지한 뒤, 인접 노트가 min_gap 이상 떨어지도록 솎아낸다."""
    onset_frames = librosa.onset.onset_detect(y=y, sr=sr, backtrack=True)
    onset_times  = librosa.frames_to_time(onset_frames, sr=sr).tolist()

    kept = []
    last = -1e9
    for t in sorted(onset_times):
        if t - last >= min_gap:
            kept.append(round(float(t), 4))
            last = t
    return kept


# ═══════════════════════════════════════════════════════════
# 2. 레인 배분 (빈도 균등 + 같은 키 3연속 제한 + 2key 동시치기)
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
# 3. 저장 & 미리보기
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
    parser = argparse.ArgumentParser(description="2key/4key 쉬움 채보 생성기")
    parser.add_argument("audio")
    parser.add_argument("--keys", type=int, choices=[2, 4], required=True)
    parser.add_argument("--difficulty", choices=["easy", "normal"], default="easy")
    parser.add_argument("--min-gap", type=float, default=None,
                        help="노트 최소 간격(초). 미지정 시 난이도 기본값(하한 0.5)")
    parser.add_argument("--chord-ratio", type=float, default=None,
                        help="2key 동시치기 비율(0~1). 미지정 시 난이도 기본값")
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--output", default=None)
    args = parser.parse_args()

    cfg     = DIFFICULTY[args.difficulty]
    min_gap = max(0.5, args.min_gap if args.min_gap is not None else cfg["min_gap"])
    chord   = args.chord_ratio if args.chord_ratio is not None else cfg["chord_ratio"]
    approach = cfg["approach"]

    print(f"[1/3] 오디오 로딩: {args.audio}")
    y, sr = librosa.load(args.audio, sr=None, mono=True)
    tempo, _ = librosa.beat.beat_track(y=y, sr=sr)
    bpm = float(tempo) if np.isscalar(tempo) else float(tempo[0])

    print(f"[2/3] 타이밍 후보 생성 (min_gap={min_gap:.2f}s)...")
    times = build_times(y, sr, min_gap)
    print(f"  ↳ 노트 시각 {len(times)}개")

    print(f"[3/3] 레인 배분 ({args.keys}key)...")
    events, counts = assign_lanes(times, args.keys, chord if args.keys == 2 else 0.0, args.seed)

    notes  = to_notes(events)
    output = args.output or (args.audio.rsplit(".", 1)[0] + f"_{args.keys}k.json")

    print_summary(events, counts, args.keys)
    save_chart(notes, args.audio, args.keys, args.difficulty, approach, bpm, output)
