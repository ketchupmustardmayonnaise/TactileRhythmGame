"""
채보 시각화 플레이어 (2 / 4 / 6key)
────────────────────────────────────────────────────────────
레인 수를 채보에서 자동 감지해 2key / 4key / 6key 모두 보여준다.
- difficulty 필드가 "2k-...", "4k-..." 이면 그 값을 사용 (lite 생성기 출력)
- 아니면 노트의 최대 lane 값으로 추정
- --keys 로 직접 지정 가능

사용법:
    python chart_visualizer.py <chart.json> <audio_file> [--keys 2|4|6]

조작:
    SPACE       일시정지 / 재개
    ← →         5초 뒤로 / 앞으로
    ESC / Q     종료
"""

import sys, os, re, json, math, time, argparse
import pygame

# ══════════════════════════════════════════════════════════
# 설정
# ══════════════════════════════════════════════════════════

W, H          = 900, 700          # 창 크기
FPS           = 60
NOTE_SPEED    = 300               # px/초  (노트 낙하 속도)
JUDGE_Y       = H - 120           # 판정선 Y
SPAWN_Y       = -40               # 노트 스폰 Y (화면 위)
NOTE_W        = 80                # 노트 너비
NOTE_H        = 22                # 노트 높이
FLASH_MS      = 120               # 판정선 플래시 지속 시간(ms)
LOOK_AHEAD    = (JUDGE_Y - SPAWN_Y) / NOTE_SPEED   # 몇 초 앞 노트까지 스폰

LANE_MARGIN   = 60                # 양쪽 여백
GAP           = 24                # 왼손/오른손 그룹 사이 간격

# 색상
BG          = (18, 18, 26)
LANE_BG     = (28, 28, 40)
LANE_BORDER = (50, 50, 70)
JUDGE_LINE  = (220, 220, 255)
DIVIDER     = (80, 80, 120)

# 왼손(쿨톤) / 오른손(웜톤) 팔레트 (각 최대 3개)
COOLS = [(80, 140, 255), (110, 180, 255), (150, 215, 255)]
WARMS = [(255, 180, 80), (255, 130, 110), (255, 90, 160)]

# 모드별 키 힌트 (게임 기본값; 표시용)
KEY_HINTS = {
    2: ["D", "K"],
    4: ["W", "S", "I", "K"],
    6: ["S", "D", "F", "J", "K", "L"],
}


# ══════════════════════════════════════════════════════════
# 채보 로드 & 레인 수 감지
# ══════════════════════════════════════════════════════════

def load_chart(json_path):
    with open(json_path, encoding="utf-8") as f:
        data = json.load(f)
    notes = [(n["time"], n["lane"]) for n in data["notes"]]
    meta  = data.get("meta", {})
    return notes, meta, data


def detect_key_count(data, notes, override):
    """레인 수 결정: --keys > difficulty 접두("4k-...") > 노트 최대 lane."""
    if override in (2, 4, 6):
        return override
    m = re.match(r"\s*(\d+)k", str(data.get("difficulty", "")))
    if m:
        return int(m.group(1))
    if notes:
        return max(l for _, l in notes)
    return 6


def build_lane_centers(n):
    """n개 레인의 중심 X 좌표. 왼손/오른손(절반) 사이에 GAP."""
    half   = n // 2
    usable = W - LANE_MARGIN * 2 - (GAP if half > 0 else 0)
    lane_w = usable / n
    centers = []
    for i in range(n):
        group_offset = GAP if i >= half else 0
        cx = LANE_MARGIN + (i + 0.5) * lane_w + group_offset
        centers.append(int(cx))
    return centers


def build_palette(n):
    """레인별 색상: 왼쪽 절반 쿨톤, 오른쪽 절반 웜톤."""
    half = n // 2
    colors = []
    for i in range(n):
        if i < half:
            colors.append(COOLS[i] if i < len(COOLS) else COOLS[-1])
        else:
            j = i - half
            colors.append(WARMS[j] if j < len(WARMS) else WARMS[-1])
    glows = [tuple(int(c * 0.55) for c in col) for col in colors]
    return colors, glows


# ══════════════════════════════════════════════════════════
# 유틸
# ══════════════════════════════════════════════════════════

def fmt_time(sec):
    sec = max(0.0, sec)
    m, s = divmod(int(sec), 60)
    return f"{m}:{s:02d}"


def draw_rounded_rect(surf, color, rect, radius=6):
    pygame.draw.rect(surf, color, rect, border_radius=radius)


def draw_glow(surf, color, rect, radius=6, spread=6):
    glow_rect = pygame.Rect(
        rect.x - spread, rect.y - spread,
        rect.width + spread * 2, rect.height + spread * 2
    )
    glow_surf = pygame.Surface((glow_rect.width, glow_rect.height), pygame.SRCALPHA)
    pygame.draw.rect(glow_surf, (*color, 60), glow_surf.get_rect(), border_radius=radius + spread)
    surf.blit(glow_surf, glow_rect.topleft)


# ══════════════════════════════════════════════════════════
# 메인 클래스
# ══════════════════════════════════════════════════════════

class Visualizer:
    def __init__(self, chart_path, audio_path, keys_override=None):
        self.notes, self.meta, data = load_chart(chart_path)
        self.audio_path  = audio_path
        self.total_notes = len(self.notes)

        # 레인 수 자동 감지 + 레이아웃 구성
        self.lane_count = detect_key_count(data, self.notes, keys_override)
        self.half       = self.lane_count // 2
        self.lane_x     = build_lane_centers(self.lane_count)
        self.colors, self.glows = build_palette(self.lane_count)
        self.key_hints  = KEY_HINTS.get(self.lane_count,
                                        [str(i + 1) for i in range(self.lane_count)])

        self.bpm = self.meta.get("bpm", 0)
        self.spb = self.meta.get("seconds_per_beat", 0)

        self.next_spawn = 0
        self.active     = []                       # {"time","lane","y"}
        self.flash      = [0] * self.lane_count

        self.paused    = False
        self.song_time = 0.0
        self.wall_ref  = None
        self.time_ref  = 0.0
        self.duration  = 0.0

        # 동시치기(같은 time에 2개 이상) 여부 미리 계산 — 연결선 표시용
        seen = {}
        for t, _ in self.notes:
            k = round(t, 3)
            seen[k] = seen.get(k, 0) + 1
        self.chord_times = {k for k, v in seen.items() if v > 1}

        print(f"[감지] {self.lane_count}-key 채보  (노트 {self.total_notes}개"
              f"{', 동시치기 있음' if self.chord_times else ''})")

    # ── 초기화 ───────────────────────────────────────────
    def init_pygame(self):
        pygame.init()
        pygame.mixer.init(frequency=44100, size=-16, channels=2, buffer=512)
        self.screen = pygame.display.set_mode((W, H))
        pygame.display.set_caption(f"Chart Visualizer — {self.lane_count}key")
        self.clock = pygame.time.Clock()

        self.font_sm = pygame.font.SysFont("Arial", 13)
        self.font_md = pygame.font.SysFont("Arial", 16, bold=True)
        self.font_lg = pygame.font.SysFont("Arial", 22, bold=True)
        self.font_xl = pygame.font.SysFont("Arial", 32, bold=True)

        try:
            pygame.mixer.music.load(self.audio_path)
            snd = pygame.mixer.Sound(self.audio_path)
            self.duration = snd.get_length()
            del snd
        except Exception as e:
            print(f"[경고] 오디오 로드 실패: {e}")
            self.duration = self.notes[-1][0] + 5.0 if self.notes else 60.0

    # ── 재생 / 시간 ──────────────────────────────────────
    def start_playback(self):
        pygame.mixer.music.play(start=self.song_time)
        self.wall_ref = pygame.time.get_ticks()
        self.time_ref = self.song_time
        self.paused   = False

    def get_song_time(self):
        if self.paused:
            return self.song_time
        elapsed = (pygame.time.get_ticks() - self.wall_ref) / 1000.0
        return self.time_ref + elapsed

    def seek(self, delta):
        self.song_time = max(0.0, min(self.get_song_time() + delta, self.duration))
        self.active.clear()
        self.next_spawn = 0
        for i, (t, _) in enumerate(self.notes):
            if t >= self.song_time - 0.1:
                self.next_spawn = i
                break
        else:
            self.next_spawn = len(self.notes)
        if not self.paused:
            pygame.mixer.music.play(start=self.song_time)
            self.wall_ref = pygame.time.get_ticks()
            self.time_ref = self.song_time

    # ── 노트 업데이트 ────────────────────────────────────
    def update_notes(self, song_time, dt):
        while self.next_spawn < len(self.notes):
            t, lane = self.notes[self.next_spawn]
            if t <= song_time + LOOK_AHEAD:
                y = JUDGE_Y - (t - song_time) * NOTE_SPEED
                self.active.append({"time": t, "lane": lane, "y": float(y)})
                self.next_spawn += 1
            else:
                break

        remove = []
        for note in self.active:
            note["y"] += NOTE_SPEED * dt
            if note["y"] > JUDGE_Y:
                li = note["lane"] - 1
                if 0 <= li < self.lane_count:
                    self.flash[li] = FLASH_MS
                remove.append(note)
        for n in remove:
            self.active.remove(n)

        self.flash = [max(0, v - dt * 1000) for v in self.flash]

    # ── BPM 비트 라인 ────────────────────────────────────
    def get_beat_lines(self, song_time):
        if self.spb <= 0:
            return []
        lines = []
        t_top = song_time - (JUDGE_Y - SPAWN_Y) / NOTE_SPEED
        i_start = math.ceil(t_top / self.spb)
        i_end   = math.ceil(song_time / self.spb)
        for i in range(i_start, i_end + 1):
            y = JUDGE_Y - (i * self.spb - song_time) * NOTE_SPEED
            if SPAWN_Y <= y <= JUDGE_Y:
                lines.append((y, i % 4 == 0))
        return lines

    # ── 그리기 ───────────────────────────────────────────
    def draw(self, song_time):
        self.screen.fill(BG)

        for by, is_strong in self.get_beat_lines(song_time):
            color = (55, 55, 80) if is_strong else (38, 38, 55)
            pygame.draw.line(self.screen, color,
                             (LANE_MARGIN, int(by)), (W - LANE_MARGIN, int(by)),
                             2 if is_strong else 1)

        # 레인 배경
        for i in range(self.lane_count):
            cx = self.lane_x[i]
            rect = pygame.Rect(cx - NOTE_W // 2 - 2, 0, NOTE_W + 4, H)
            pygame.draw.rect(self.screen, LANE_BG, rect)
            pygame.draw.rect(self.screen, LANE_BORDER, rect, 1)

        # 왼손/오른손 구분선 (레인이 4개 이상이고 절반이 있을 때)
        if 0 < self.half < self.lane_count:
            mid_x = (self.lane_x[self.half - 1] + self.lane_x[self.half]) // 2
            pygame.draw.line(self.screen, DIVIDER, (mid_x, 0), (mid_x, JUDGE_Y + 10), 2)

        # 동시치기 연결선 (같은 time의 활성 노트끼리)
        self._draw_chord_links()

        # 노트
        for note in self.active:
            li = note["lane"] - 1
            if not (0 <= li < self.lane_count):
                continue
            cx = self.lane_x[li]
            y  = int(note["y"])
            rect = pygame.Rect(cx - NOTE_W // 2, y - NOTE_H // 2, NOTE_W, NOTE_H)
            if SPAWN_Y - NOTE_H <= y <= H:
                draw_glow(self.screen, self.glows[li], rect)
                draw_rounded_rect(self.screen, self.colors[li], rect, radius=5)
                hi_rect = pygame.Rect(rect.x + 4, rect.y + 3, rect.width - 8, 3)
                pygame.draw.rect(self.screen, (255, 255, 255, 80), hi_rect, border_radius=2)

        # 판정선
        pygame.draw.line(self.screen, JUDGE_LINE,
                         (LANE_MARGIN - 10, JUDGE_Y), (W - LANE_MARGIN + 10, JUDGE_Y), 2)

        # 판정선 버튼 & 플래시
        for i in range(self.lane_count):
            cx = self.lane_x[i]
            color = self.colors[i]
            btn_rect = pygame.Rect(cx - NOTE_W // 2, JUDGE_Y - NOTE_H // 2, NOTE_W, NOTE_H)
            base_c = tuple(int(c * 0.35) for c in color)
            draw_rounded_rect(self.screen, base_c, btn_rect, radius=5)
            alpha = min(1.0, self.flash[i] / FLASH_MS)
            if alpha > 0:
                flash_surf = pygame.Surface((NOTE_W, NOTE_H), pygame.SRCALPHA)
                pygame.draw.rect(flash_surf, (*color, int(200 * alpha)),
                                 flash_surf.get_rect(), border_radius=5)
                self.screen.blit(flash_surf, btn_rect.topleft)

        # 레인 라벨: 번호 + 키 힌트
        for i in range(self.lane_count):
            cx = self.lane_x[i]
            t1 = self.font_md.render(str(i + 1), True, self.colors[i])
            t2 = self.font_sm.render(self.key_hints[i], True, (170, 170, 190))
            self.screen.blit(t1, t1.get_rect(centerx=cx, centery=JUDGE_Y + 32))
            self.screen.blit(t2, t2.get_rect(centerx=cx, centery=JUDGE_Y + 50))

        self._draw_hud(song_time)

    def _draw_chord_links(self):
        """같은 time에 걸린 활성 노트들을 옅은 선으로 연결(동시치기 표시)."""
        if not self.chord_times:
            return
        groups = {}
        for note in self.active:
            k = round(note["time"], 3)
            if k in self.chord_times:
                groups.setdefault(k, []).append(note)
        for k, ns in groups.items():
            if len(ns) < 2:
                continue
            ns_sorted = sorted(ns, key=lambda n: n["lane"])
            y = int(sum(n["y"] for n in ns) / len(ns))
            x1 = self.lane_x[ns_sorted[0]["lane"] - 1]
            x2 = self.lane_x[ns_sorted[-1]["lane"] - 1]
            pygame.draw.line(self.screen, (255, 255, 255), (x1, y), (x2, y), 1)

    def _draw_hud(self, song_time):
        bar_x, bar_y = 60, H - 28
        bar_w, bar_h = W - 120, 8
        pygame.draw.rect(self.screen, (50, 50, 70), (bar_x, bar_y, bar_w, bar_h), border_radius=4)
        if self.duration > 0:
            fill_w = int(bar_w * min(1.0, song_time / self.duration))
            if fill_w > 0:
                pygame.draw.rect(self.screen, (120, 160, 255), (bar_x, bar_y, fill_w, bar_h), border_radius=4)

        time_str = f"{fmt_time(song_time)} / {fmt_time(self.duration)}"
        bpm_str  = f"BPM {self.bpm:.1f}" if self.bpm else ""
        self.screen.blit(self.font_md.render(time_str, True, (200, 200, 220)), (bar_x, H - 52))
        t_bpm = self.font_md.render(bpm_str, True, (160, 160, 200))
        self.screen.blit(t_bpm, (W - 60 - t_bpm.get_width(), H - 52))

        if self.paused:
            t_pause = self.font_xl.render("PAUSED", True, (255, 200, 80))
            self.screen.blit(t_pause, t_pause.get_rect(center=(W // 2, H // 2)))

        hint = "SPACE: 일시정지  |  ← →: ±5초  |  Q: 종료"
        t_hint = self.font_sm.render(hint, True, (90, 90, 110))
        self.screen.blit(t_hint, t_hint.get_rect(centerx=W // 2, centery=H - 14))

        passed = sum(1 for t, _ in self.notes if t < song_time)
        count_str = f"{self.lane_count}K   Notes {passed} / {self.total_notes}"
        t_count = self.font_md.render(count_str, True, (160, 180, 220))
        self.screen.blit(t_count, (W // 2 - t_count.get_width() // 2, 14))

        # 그룹 라벨 (6key만 저/고음, 나머지는 왼손/오른손)
        if 0 < self.half < self.lane_count:
            if self.lane_count == 6:
                left_lbl, right_lbl = "◀ 저음", "고음 ▶"
            else:
                left_lbl, right_lbl = "◀ 왼손", "오른손 ▶"
            self.screen.blit(self.font_sm.render(left_lbl, True, (100, 160, 255)),
                             (LANE_MARGIN, 14))
            t_r = self.font_sm.render(right_lbl, True, (255, 160, 100))
            self.screen.blit(t_r, (W - LANE_MARGIN - t_r.get_width(), 14))

    # ── 메인 루프 ────────────────────────────────────────
    def run(self):
        self.init_pygame()
        self.start_playback()

        while True:
            dt = self.clock.tick(FPS) / 1000.0
            now = pygame.time.get_ticks()

            for event in pygame.event.get():
                if event.type == pygame.QUIT:
                    pygame.quit(); sys.exit()
                if event.type == pygame.KEYDOWN:
                    if event.key in (pygame.K_ESCAPE, pygame.K_q):
                        pygame.quit(); sys.exit()
                    elif event.key == pygame.K_SPACE:
                        if self.paused:
                            pygame.mixer.music.unpause()
                            self.wall_ref = now
                            self.time_ref = self.song_time
                            self.paused = False
                        else:
                            self.song_time = self.get_song_time()
                            pygame.mixer.music.pause()
                            self.paused = True
                    elif event.key == pygame.K_LEFT:
                        self.seek(-5.0)
                    elif event.key == pygame.K_RIGHT:
                        self.seek(+5.0)

            song_time = self.get_song_time()
            if not self.paused:
                self.song_time = song_time
                self.update_notes(song_time, dt)

            self.draw(song_time)
            pygame.display.flip()

            if not self.paused and song_time >= self.duration + 1.0:
                time.sleep(0.5)
                pygame.quit(); sys.exit()


# ══════════════════════════════════════════════════════════
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="채보 시각화 플레이어 (2/4/6key)")
    parser.add_argument("chart")
    parser.add_argument("audio")
    parser.add_argument("--keys", type=int, choices=[2, 4, 6], default=None,
                        help="레인 수 강제 지정(미지정 시 자동 감지)")
    args = parser.parse_args()

    if not os.path.exists(args.chart):
        sys.exit(f"채보 파일 없음: {args.chart}")

    Visualizer(args.chart, args.audio, args.keys).run()