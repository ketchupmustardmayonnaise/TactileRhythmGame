"""기존 *_2k.json의 음악 타이밍을 유지하며 Easy / Single 채보를 생성한다.

python Python/chart_generator_lite_grid.py --test-modes [--min-gap 1.25]
원본은 Unity의 Assets/Resources/Songs이며 *_2k.json은 덮어쓰지 않는다.
Python 표준 라이브러리만 사용한다.
"""

import argparse
import copy
import json
import math
from pathlib import Path

DEFAULT_SONGS = Path(__file__).resolve().parents[1] / "Assets" / "Resources" / "Songs"
MODES = {"easy": 2, "single": 1}


def make_test_charts(source, min_gap=1.25):
    """최소 1초 간격, 동시치기 없음. 두 모드의 타이밍을 같게 해 비교한다."""
    if not math.isfinite(min_gap) or min_gap < 1.0:
        raise ValueError("min_gap은 1.0초 이상의 유한한 값이어야 합니다.")
    if not isinstance(source.get("source"), str) or not source["source"]:
        raise ValueError("원본 채보에 오디오 source가 필요합니다.")
    notes = source.get("notes")
    if not isinstance(notes, list) or not notes:
        raise ValueError("원본 채보에 노트가 없습니다.")

    candidates = {}
    for note in notes:
        time = note.get("time")
        lane = note.get("lane")
        if (isinstance(time, bool) or not isinstance(time, (int, float))
                or not math.isfinite(time) or time < 0):
            raise ValueError(f"잘못된 노트 시각: {time!r}")
        if isinstance(lane, bool) or lane not in (1, 2):
            raise ValueError(f"원본은 2key 채보여야 합니다: lane={lane!r}")
        candidates.setdefault(time, set()).add(lane)

    kept = []
    counts = {1: 0, 2: 0}
    last_time = -math.inf
    for time, lanes in sorted(candidates.items()):
        if time - last_time < min_gap:
            continue
        # 동시치기라면 덜 사용된 한쪽만 남긴다. 단일 노트의 레인은 그대로 둔다.
        lane = min(lanes, key=lambda value: (counts[value], value))
        counts[lane] += 1
        kept.append({"time": time, "lane": lane})
        last_time = time

    result = {}
    for mode, keys in MODES.items():
        chart = copy.deepcopy(source)
        chart["difficulty"] = mode
        chart["meta"] = dict(chart.get("meta") or {})
        chart["meta"].update({
            "seconds_per_beat": 0.3,  # 기존 로더의 예고 시간 메타데이터 규약
            "test_mode": mode,
            "keys": keys,
            "min_gap_seconds": min_gap,
            "source_note_count": len(notes),
        })
        chart["notes"] = [
            {"time": note["time"], "lane": note["lane"] if keys == 2 else 1}
            for note in kept
        ]
        result[mode] = chart
    return result


def generate(source_dir, output_dir, min_gap=1.25):
    sources = sorted(Path(source_dir).glob("*_2k.json"))
    if not sources:
        raise ValueError(f"*_2k.json 원본을 찾지 못했습니다: {source_dir}")
    # 모든 원본을 먼저 검증해 잘못된 원본 때문에 일부 곡만 갱신되는 일을 막는다.
    generated = []
    for path in sources:
        source = json.loads(path.read_text(encoding="utf-8-sig"))
        charts = make_test_charts(source, min_gap)
        for mode, chart in charts.items():
            destination = Path(output_dir) / f"{path.stem[:-3]}_{mode}.json"
            generated.append((destination, chart))
    Path(output_dir).mkdir(parents=True, exist_ok=True)
    for destination, chart in generated:
        destination.write_text(json.dumps(chart, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"{destination.name}: {len(chart['notes'])} notes "
              f"(source {chart['meta']['source_note_count']}, gap >= {min_gap:g}s)")
    return generated


def main(argv=None):
    parser = argparse.ArgumentParser(description="Classic 원본을 보존하고 Easy/Single 채보를 생성합니다.")
    parser.add_argument("--source-dir", type=Path, default=DEFAULT_SONGS)
    parser.add_argument("--output-dir", type=Path, default=None,
                        help="기본값: 원본 폴더. Unity에서 읽을 위치는 Assets/Resources/Songs")
    parser.add_argument("--min-gap", type=float, default=1.25, help="노트 최소 간격(초), 하한 1.0")
    args = parser.parse_args(argv)
    try:
        generate(args.source_dir, args.output_dir or args.source_dir, args.min_gap)
    except (ValueError, OSError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
