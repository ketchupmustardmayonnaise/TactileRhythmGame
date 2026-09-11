import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from chart_test_modes import DEFAULT_SONGS, generate, make_test_charts


class TestCharts(unittest.TestCase):
    def setUp(self):
        self.source = {
            "source": "sample.mp3", "difficulty": "2k-easy",
            "meta": {"bpm": 120, "seconds_per_beat": 0.5},
            "notes": [
                {"time": 0, "lane": 1}, {"time": 0, "lane": 2},
                {"time": 0.5, "lane": 2}, {"time": 1.25, "lane": 2},
                {"time": 2.0, "lane": 1}, {"time": 2.5, "lane": 1},
            ],
        }

    def test_chords_removed_and_common_timeline(self):
        charts = make_test_charts(self.source)
        self.assertEqual([n["time"] for n in charts["easy"]["notes"]], [0, 1.25, 2.5])
        self.assertEqual([n["time"] for n in charts["single"]["notes"]], [0, 1.25, 2.5])
        self.assertEqual([n["lane"] for n in charts["easy"]["notes"]], [1, 2, 1])
        self.assertEqual({n["lane"] for n in charts["single"]["notes"]}, {1})

    def test_source_unchanged_and_results_independent(self):
        original = copy.deepcopy(self.source)
        charts = make_test_charts(self.source)
        charts["easy"]["meta"]["bpm"] = 0
        charts["easy"]["notes"][0]["time"] = 50
        self.assertEqual(self.source, original)
        self.assertEqual(charts["single"]["meta"]["bpm"], 120)
        self.assertEqual(charts["single"]["notes"][0]["time"], 0)

    def test_invalid_gap_rejected(self):
        for gap in (0.9, -1, float("nan"), float("inf")):
            with self.subTest(gap=gap), self.assertRaises(ValueError):
                make_test_charts(self.source, gap)

    def test_invalid_lane_time_and_empty_chart_rejected(self):
        for note in ({"time": -1, "lane": 1}, {"time": 1, "lane": 3},
                     {"time": float("nan"), "lane": 1}):
            with self.subTest(note=note), self.assertRaises(ValueError):
                make_test_charts({**self.source, "notes": [note]})
        with self.assertRaises(ValueError):
            make_test_charts({**self.source, "notes": []})

    def test_unsorted_input(self):
        reverse = {**self.source, "notes": list(reversed(self.source["notes"]))}
        self.assertEqual(make_test_charts(reverse), make_test_charts(self.source))

    def test_all_shipped_songs(self):
        sources = sorted(DEFAULT_SONGS.glob("*_2k.json"))
        self.assertEqual(len(sources), 5)
        for path in sources:
            source = json.loads(path.read_text(encoding="utf-8-sig"))
            times = {n["time"] for n in source["notes"]}
            charts = make_test_charts(source)
            for mode, chart in charts.items():
                with self.subTest(song=path.stem, mode=mode):
                    notes = chart["notes"]
                    self.assertTrue(notes)
                    self.assertLess(len(notes), len(source["notes"]))
                    self.assertTrue(all(b["time"] - a["time"] >= 1.25
                                        for a, b in zip(notes, notes[1:])))
                    self.assertTrue(all(n["time"] in times for n in notes))
                    self.assertEqual(chart["source"], source["source"])
                    self.assertEqual({n["lane"] for n in notes}, {1, 2} if mode == "easy" else {1})
                    shipped = path.with_name(path.stem[:-3] + f"_{mode}.json")
                    self.assertEqual(json.loads(shipped.read_text(encoding="utf-8")), chart)

    def test_regeneration_is_reproducible_and_preserves_original(self):
        # 임시 파일도 프로젝트 내부에만 생성한다.
        scratch = Path(__file__).resolve().parents[1] / "Temp" / "ModeValidation"
        scratch.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=scratch) as folder:
            path = Path(folder) / "sample_2k.json"
            path.write_text(json.dumps(self.source), encoding="utf-8")
            original = hashlib.sha256(path.read_bytes()).hexdigest()
            generate(folder, folder)
            first = {p.name: p.read_bytes() for p in Path(folder).glob("*.json")}
            generate(folder, folder)
            self.assertEqual({p.name: p.read_bytes() for p in Path(folder).glob("*.json")}, first)
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), original)


if __name__ == "__main__":
    unittest.main()
