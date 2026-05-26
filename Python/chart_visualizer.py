"""
채보 시각화 플레이어
────────────────────────────────────────────────────────────
사용법:
    python chart_visualizer.py <chart.json> <audio_file>

조작:
    SPACE       일시정지 / 재개
    ← →         5초 뒤로 / 앞으로
    ESC / Q     종료
"""

import sys, json, math, time
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

# 레인 배치 ─ 가운데 정렬, 1~3 / 4~6 사이에 간격
LANE_MARGIN   = 60                # 양쪽 여백
GAP           = 24                # 저음/고음 그룹 사이 간격
LANE_COUNT    = 6

def build_lane_centers():
    """각 레인의 중심 X 좌표 계산"""
    usable = W - LANE_MARGIN * 2 - GAP
    lane_w = usable / LANE_COUNT
    centers = []
    for i in range(LANE_COUNT):
        group_offset = GAP if i >= 3 else 0
        cx = LANE_MARGIN + (i + 0.5) * lane_w + group_offset
        centers.append(int(cx))
    return centers

LANE_X = build_lane_centers()

# 색상
BG          = (18, 18, 26)
LANE_BG     = (28, 28, 40)
LANE_BORDER = (50, 50, 70)
JUDGE_LINE  = (220, 220, 255)
DIVIDER     = (80, 80, 120)

# 레인별 색상 (저음 = 파랑 계열, 고음 = 붉은/노랑 계열)
NOTE_COLORS = [
    (80,  140, 255),   # 레인 1 - 딥블루
    (100, 180, 255),   # 레인 2 - 블루
    (140, 220, 255),   # 레인 3 - 하늘
    (255, 180,  80),   # 레인 4 - 주황
    (255, 120, 120),   # 레인 5 - 연빨강
    (255,  80, 160),   # 레인 6 - 핑크
]
GLOW_COLORS = [
    (40,  80, 180),
    (50, 100, 180),
    (70, 140, 180),
    (180, 100,  30),
    (180,  60,  60),
    (180,  30, 100),
]
LANE_LABEL  = ["1\n저", "2\n저", "3\n저", "4\n고", "5\n고", "6\n고"]


# ══════════════════════════════════════════════════════════
# 유틸
# ══════════════════════════════════════════════════════════

def load_chart(json_path):
    with open(json_path, encoding="utf-8") as f:
        data = json.load(f)
    notes = [(n["time"], n["lane"]) for n in data["notes"]]
    meta  = data.get("meta", {})
    return notes, meta


def fmt_time(sec):
    sec = max(0.0, sec)
    m, s = divmod(int(sec), 60)
    return f"{m}:{s:02d}"


def draw_rounded_rect(surf, color, rect, radius=6):
    pygame.draw.rect(surf, color, rect, border_radius=radius)


def draw_glow(surf, color, rect, radius=6, spread=6):
    """노트 주위 글로우 효과"""
    glow_rect = pygame.Rect(
        rect.x - spread, rect.y - spread,
        rect.width + spread * 2, rect.height + spread * 2
    )
    glow_surf = pygame.Surface((glow_rect.width, glow_rect.height), pygame.SRCALPHA)
    gc = (*color, 60)
    pygame.draw.rect(glow_surf, gc, glow_surf.get_rect(), border_radius=radius + spread)
    surf.blit(glow_surf, glow_rect.topleft)


# ══════════════════════════════════════════════════════════
# 메인 클래스
# ══════════════════════════════════════════════════════════

class Visualizer:
    def __init__(self, chart_path, audio_path):
        self.notes, self.meta = load_chart(chart_path)
        self.audio_path = audio_path
        self.total_notes = len(self.notes)

        self.bpm = self.meta.get("bpm", 0)
        self.spb = self.meta.get("seconds_per_beat", 0)

        # 노트 인덱스 (아직 스폰 안 된 첫 번째)
        self.next_spawn = 0

        # 활성 노트: {"time", "lane", "y"}
        self.active = []

        # 판정선 플래시: {lane_idx: 남은ms}
        self.flash = [0] * LANE_COUNT

        # 재생 상태
        self.paused    = False
        self.song_time = 0.0          # 현재 노래 시간(초)
        self.wall_ref  = None         # pygame.time.get_ticks() 기준점
        self.time_ref  = 0.0          # wall_ref 시점의 song_time

        # 노래 총 길이
        self.duration = 0.0

        # BPM 비트 라인 y 위치들
        self.beat_lines = []

    # ── 초기화 ───────────────────────────────────────────
    def init_pygame(self):
        pygame.init()
        pygame.mixer.init(frequency=44100, size=-16, channels=2, buffer=512)
        self.screen = pygame.display.set_mode((W, H))
        pygame.display.set_caption("Chart Visualizer")
        self.clock = pygame.time.Clock()

        # 폰트
        self.font_sm  = pygame.font.SysFont("Arial", 13)
        self.font_md  = pygame.font.SysFont("Arial", 16, bold=True)
        self.font_lg  = pygame.font.SysFont("Arial", 22, bold=True)
        self.font_xl  = pygame.font.SysFont("Arial", 32, bold=True)

        # 오디오 로드
        try:
            pygame.mixer.music.load(self.audio_path)
            # 임시로 길이 구하기
            snd = pygame.mixer.Sound(self.audio_path)
            self.duration = snd.get_length()
            del snd
        except Exception as e:
            print(f"[경고] 오디오 로드 실패: {e}")
            self.duration = self.notes[-1][0] + 5.0 if self.notes else 60.0

    # ── 재생 시작 ────────────────────────────────────────
    def start_playback(self):
        pygame.mixer.music.play(start=self.song_time)
        self.wall_ref = pygame.time.get_ticks()
        self.time_ref = self.song_time
        self.paused   = False

    # ── 시간 동기화 ──────────────────────────────────────
    def get_song_time(self):
        if self.paused:
            return self.song_time
        elapsed = (pygame.time.get_ticks() - self.wall_ref) / 1000.0
        return self.time_ref + elapsed

    # ── seek ─────────────────────────────────────────────
    def seek(self, delta):
        self.song_time = max(0.0, min(self.get_song_time() + delta, self.duration))
        self.active.clear()
        # next_spawn 재계산
        self.next_spawn = 0
        for i, (t, _) in enumerate(self.notes):
            if t >= self.song_time - 0.1:
                self.next_spawn = i
                break
        if not self.paused:
            pygame.mixer.music.play(start=self.song_time)
            self.wall_ref = pygame.time.get_ticks()
            self.time_ref = self.song_time

    # ── 노트 스폰 ────────────────────────────────────────
    def update_notes(self, song_time, dt):
        # 새 노트 스폰 (판정선 도달 기준 LOOK_AHEAD 초 전)
        while self.next_spawn < len(self.notes):
            t, lane = self.notes[self.next_spawn]
            if t <= song_time + LOOK_AHEAD:
                time_to_judge = t - song_time
                y = JUDGE_Y - time_to_judge * NOTE_SPEED
                self.active.append({"time": t, "lane": lane, "y": float(y)})
                self.next_spawn += 1
            else:
                break

        # 노트 이동
        remove = []
        for note in self.active:
            note["y"] += NOTE_SPEED * dt
            if note["y"] > JUDGE_Y:
                # 판정선 통과 → 플래시 & 제거
                li = note["lane"] - 1
                self.flash[li] = FLASH_MS
                remove.append(note)
        for n in remove:
            self.active.remove(n)

        # 플래시 감소
        self.flash = [max(0, v - dt * 1000) for v in self.flash]

    # ── BPM 비트 라인 ────────────────────────────────────
    def get_beat_lines(self, song_time):
        """현재 화면에 보여야 할 비트선 Y 좌표 목록"""
        if self.spb <= 0:
            return []
        lines = []
        # 화면에 표시될 시간 범위
        t_bottom = song_time
        t_top    = song_time - (JUDGE_Y - SPAWN_Y) / NOTE_SPEED

        # 첫 번째 비트 인덱스
        i_start = math.ceil(t_top / self.spb)
        i_end   = math.ceil(t_bottom / self.spb)

        for i in range(i_start, i_end + 1):
            beat_t = i * self.spb
            y = JUDGE_Y - (beat_t - song_time) * NOTE_SPEED
            if SPAWN_Y <= y <= JUDGE_Y:
                lines.append((y, i % 4 == 0))  # (y, 강박여부)
        return lines

    # ── 그리기 ───────────────────────────────────────────
    def draw(self, song_time):
        self.screen.fill(BG)

        # ── BPM 비트선 ──────────────────────────────────
        for by, is_strong in self.get_beat_lines(song_time):
            color = (55, 55, 80) if is_strong else (38, 38, 55)
            pygame.draw.line(self.screen, color, (LANE_MARGIN, int(by)), (W - LANE_MARGIN, int(by)), 1 if not is_strong else 2)

        # ── 레인 배경 ───────────────────────────────────
        for i in range(LANE_COUNT):
            cx   = LANE_X[i]
            rect = pygame.Rect(cx - NOTE_W // 2 - 2, 0, NOTE_W + 4, H)
            pygame.draw.rect(self.screen, LANE_BG, rect)
            pygame.draw.rect(self.screen, LANE_BORDER, rect, 1)

        # ── 그룹 구분선 ─────────────────────────────────
        mid_x = (LANE_X[2] + LANE_X[3]) // 2
        pygame.draw.line(self.screen, DIVIDER, (mid_x, 0), (mid_x, JUDGE_Y + 10), 2)

        # ── 노트 ────────────────────────────────────────
        for note in self.active:
            li    = note["lane"] - 1
            cx    = LANE_X[li]
            y     = int(note["y"])
            color = NOTE_COLORS[li]
            glow  = GLOW_COLORS[li]
            rect  = pygame.Rect(cx - NOTE_W // 2, y - NOTE_H // 2, NOTE_W, NOTE_H)
            if SPAWN_Y - NOTE_H <= y <= H:
                draw_glow(self.screen, glow, rect)
                draw_rounded_rect(self.screen, color, rect, radius=5)
                # 하이라이트 줄
                hi_rect = pygame.Rect(rect.x + 4, rect.y + 3, rect.width - 8, 3)
                pygame.draw.rect(self.screen, (255, 255, 255, 80), hi_rect, border_radius=2)

        # ── 판정선 ──────────────────────────────────────
        pygame.draw.line(self.screen, JUDGE_LINE, (LANE_MARGIN - 10, JUDGE_Y), (W - LANE_MARGIN + 10, JUDGE_Y), 2)

        # ── 판정선 버튼 & 플래시 ────────────────────────
        for i in range(LANE_COUNT):
            cx    = LANE_X[i]
            color = NOTE_COLORS[i]
            alpha = min(1.0, self.flash[i] / FLASH_MS)
            btn_rect = pygame.Rect(cx - NOTE_W // 2, JUDGE_Y - NOTE_H // 2, NOTE_W, NOTE_H)

            # 기본 버튼
            base_c = tuple(int(c * 0.35) for c in color)
            draw_rounded_rect(self.screen, base_c, btn_rect, radius=5)

            # 플래시 오버레이
            if alpha > 0:
                flash_surf = pygame.Surface((NOTE_W, NOTE_H), pygame.SRCALPHA)
                fc = (*color, int(200 * alpha))
                pygame.draw.rect(flash_surf, fc, flash_surf.get_rect(), border_radius=5)
                self.screen.blit(flash_surf, btn_rect.topleft)

        # ── 레인 라벨 ───────────────────────────────────
        for i in range(LANE_COUNT):
            cx = LANE_X[i]
            label = f"{i+1}"
            group = "저" if i < 3 else "고"
            t1 = self.font_md.render(label, True, NOTE_COLORS[i])
            t2 = self.font_sm.render(group, True, (160, 160, 180))
            self.screen.blit(t1, t1.get_rect(centerx=cx, centery=JUDGE_Y + 32))
            self.screen.blit(t2, t2.get_rect(centerx=cx, centery=JUDGE_Y + 50))

        # ── 하단 HUD ────────────────────────────────────
        self._draw_hud(song_time)

    def _draw_hud(self, song_time):
        # 진행 바 배경
        bar_x, bar_y = 60, H - 28
        bar_w = W - 120
        bar_h = 8
        pygame.draw.rect(self.screen, (50, 50, 70), (bar_x, bar_y, bar_w, bar_h), border_radius=4)

        # 진행 바 채우기
        if self.duration > 0:
            ratio = min(1.0, song_time / self.duration)
            fill_w = int(bar_w * ratio)
            if fill_w > 0:
                pygame.draw.rect(self.screen, (120, 160, 255), (bar_x, bar_y, fill_w, bar_h), border_radius=4)

        # BPM / 시간
        time_str = f"{fmt_time(song_time)} / {fmt_time(self.duration)}"
        bpm_str  = f"BPM {self.bpm:.1f}" if self.bpm else ""

        t_time = self.font_md.render(time_str, True, (200, 200, 220))
        t_bpm  = self.font_md.render(bpm_str,  True, (160, 160, 200))
        self.screen.blit(t_time, (bar_x, H - 52))
        self.screen.blit(t_bpm,  (W - 60 - t_bpm.get_width(), H - 52))

        # 일시정지 표시
        if self.paused:
            t_pause = self.font_xl.render("⏸ PAUSED", True, (255, 200, 80))
            self.screen.blit(t_pause, t_pause.get_rect(center=(W // 2, H // 2)))

        # 조작법 힌트
        hint = "SPACE: 일시정지  |  ← →: ±5초  |  Q: 종료"
        t_hint = self.font_sm.render(hint, True, (90, 90, 110))
        self.screen.blit(t_hint, t_hint.get_rect(centerx=W // 2, centery=H - 14))

        # 상단: 노트 카운트
        passed = sum(1 for t, _ in self.notes if t < song_time)
        count_str = f"Notes  {passed} / {self.total_notes}"
        t_count = self.font_md.render(count_str, True, (160, 180, 220))
        self.screen.blit(t_count, (W // 2 - t_count.get_width() // 2, 14))

        # 그룹 라벨
        t_low  = self.font_sm.render("◀ 저음 (1~3)", True, (100, 160, 255))
        t_high = self.font_sm.render("고음 (4~6) ▶", True, (255, 160, 100))
        self.screen.blit(t_low,  (LANE_MARGIN, 14))
        self.screen.blit(t_high, (W - LANE_MARGIN - t_high.get_width(), 14))

    # ── 메인 루프 ────────────────────────────────────────
    def run(self):
        self.init_pygame()
        self.start_playback()

        prev_ticks = pygame.time.get_ticks()

        while True:
            dt_ms = self.clock.tick(FPS)
            dt    = dt_ms / 1000.0
            now   = pygame.time.get_ticks()

            # ── 이벤트 처리 ─────────────────────────────
            for event in pygame.event.get():
                if event.type == pygame.QUIT:
                    pygame.quit(); sys.exit()

                if event.type == pygame.KEYDOWN:
                    if event.key in (pygame.K_ESCAPE, pygame.K_q):
                        pygame.quit(); sys.exit()

                    elif event.key == pygame.K_SPACE:
                        if self.paused:
                            # 재개
                            pygame.mixer.music.unpause()
                            self.wall_ref = now
                            self.time_ref = self.song_time
                            self.paused   = False
                        else:
                            # 일시정지
                            self.song_time = self.get_song_time()
                            pygame.mixer.music.pause()
                            self.paused = True

                    elif event.key == pygame.K_LEFT:
                        self.seek(-5.0)
                    elif event.key == pygame.K_RIGHT:
                        self.seek(+5.0)

            # ── 시간 업데이트 ────────────────────────────
            song_time = self.get_song_time()
            if not self.paused:
                self.song_time = song_time

            # ── 노트 업데이트 ────────────────────────────
            if not self.paused:
                self.update_notes(song_time, dt)

            # ── 그리기 ───────────────────────────────────
            self.draw(song_time)
            pygame.display.flip()

            # ── 곡 끝 ────────────────────────────────────
            if not self.paused and song_time >= self.duration + 1.0:
                time.sleep(0.5)
                pygame.quit(); sys.exit()


# ══════════════════════════════════════════════════════════
if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("사용법: python chart_visualizer.py <chart.json> <audio_file>")
        sys.exit(1)

    chart_path = sys.argv[1]
    audio_path = sys.argv[2]

    Visualizer(chart_path, audio_path).run()
