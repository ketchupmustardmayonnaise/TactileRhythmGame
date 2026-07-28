"""
채보 수동 입력 레코더 (2 / 4key)
────────────────────────────────────────────────────────────
음악을 틀어놓고, 타이밍에 맞춰 키를 누르면 그 시각이 노트로 기록된다.
녹음이 끝나면 생성기/비주얼라이저와 동일한 형식의 채보 JSON을 저장한다.

키 매핑(비주얼라이저 KEY_HINTS와 동일):
    2key :  D → 레인1 ,  K → 레인2
    4key :  W → 레인1 ,  S → 레인2 ,  I → 레인3 ,  K → 레인4

사용법:
    python chart_recorder.py <오디오파일> --keys 2
    python chart_recorder.py song.mp3 --keys 4 --output song_4k_manual.json
    python chart_recorder.py song.mp3 --keys 4 --offset 80   # 입력 지연 80ms 보정

조작:
    (레인 키)    현재 재생 시각에 노트 기록
    SPACE        일시정지 / 재개  (정지 중에는 기록 안 됨)
    Z / BKSP     마지막 기록 취소(되돌리기)
    ← →          5초 뒤로 / 앞으로
    ENTER        지금까지 기록을 파일로 저장(계속 녹음 가능)
    ESC / Q      저장하고 종료

출력 JSON은 게임 SongLoader / chart_visualizer.py가 읽는 형식과 동일하다.
"""

import sys, os, json, argparse
import pygame

# ══════════════════════════════════════════════════════════
# 설정 (비주얼라이저와 시각을 맞춤)
# ══════════════════════════════════════════════════════════

W, H          = 900, 700
FPS           = 240               # 입력 타이밍 양자화 오차를 줄이려 높게 폴링
JUDGE_Y       = H - 120           # 판정선 Y (여기서 키를 누른다고 상상)
NOTE_W        = 80
NOTE_H        = 22
FLASH_MS      = 130               # 키 입력 시 판정 버튼 플래시 지속(ms)
MARKER_SPEED  = 300               # 기록된 노트가 위로 떠오르는 속도(px/초)

LANE_MARGIN   = 60
GAP           = 24

# 색상
BG          = (18, 18, 26)
LANE_BG     = (28, 28, 40)
LANE_BORDER = (50, 50, 70)
JUDGE_LINE  = (220, 220, 255)
DIVIDER     = (80, 80, 120)

COOLS = [(80, 140, 255), (110, 180, 255), (150, 215, 255)]
WARMS = [(255, 180, 80), (255, 130, 110), (255, 90, 160)]

# 모드별 레인 → 키 매핑 (1-based 레인 순서대로)
KEY_LAYOUT = {
    2: ["D", "K"],
    4: ["W", "S", "I", "K"],
}
KEY_TO_PYGAME = {
    "D": pygame.K_d, "K": pygame.K_k,
    "W": pygame.K_w, "S": pygame.K_s, "I": pygame.K_i,
}


# ══════════════════════════════════════════════════════════
# 레이아웃 / 팔레트 (비주얼라이저와 동일 로직)
# ══════════════════════════════════════════════════════════

def build_lane_centers(n):
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
    half = n // 2
    colors = []
    for i in range(n):
        if i < half:
            colors.append(COOLS[i] if i < len(COOLS) else COOLS[-1])
        else:
            j = i - half
            colors.append(WARMS[j] if j < len(WARMS) else WARMS[-1])
    return colors


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
    glow_rect = pygame.Rect(rect.x - spread, rect.y - spread,
                            rect.width + spread * 2, rect.height + spread * 2)
    glow_surf = pygame.Surface((glow_rect.width, glow_rect.height), pygame.SRCALPHA)
    pygame.draw.rect(glow_surf, (*color, 60), glow_surf.get_rect(),
                     border_radius=radius + spread)
    surf.blit(glow_surf, glow_rect.topleft)


# ══════════════════════════════════════════════════════════
# 레코더 본체
# ══════════════════════════════════════════════════════════

class Recorder:
    def __init__(self, audio_path, keys, offset_ms, approach, bpm,
                 difficulty, countin, output_path):
        self.audio_path  = audio_path
        self.lane_count  = keys
        self.offset      = offset_ms / 1000.0     # 기록 시각에서 빼줄 지연 보정(초)
        self.approach    = approach               # meta.seconds_per_beat (게임 previewWindow)
        self.bpm         = bpm
        self.difficulty  = difficulty
        self.countin     = max(0, countin)
        self.output_path = output_path

        self.half     = self.lane_count // 2
        self.lane_x   = build_lane_centers(self.lane_count)
        self.colors   = build_palette(self.lane_count)
        self.hints    = KEY_LAYOUT[self.lane_count]

        # 레인 키(파이게임 키코드) → 1-based 레인 번호
        self.key_to_lane = {KEY_TO_PYGAME[h]: i + 1 for i, h in enumerate(self.hints)}

        self.recorded = []          # [{"time","lane"}, ...] (입력 순서대로 쌓임)
        self.markers  = []          # 화면 피드백용 {"lane","y","alpha"}
        self.flash    = [0] * self.lane_count

        self.state     = "countin" if self.countin > 0 else "play"
        self.paused    = False
        self.song_time = 0.0
        self.wall_ref  = None
        self.time_ref  = 0.0
        self.duration  = 0.0
        self.countin_start = None

    # ── 초기화 ───────────────────────────────────────────
    def init_pygame(self):
        pygame.init()
        pygame.mixer.init(frequency=44100, size=-16, channels=2, buffer=512)
        self.screen = pygame.display.set_mode((W, H))
        pygame.display.set_caption(f"Chart Recorder — {self.lane_count}key")
        self.clock = pygame.time.Clock()

        self.font_sm = pygame.font.SysFont("Arial", 13)
        self.font_md = pygame.font.SysFont("Arial", 16, bold=True)
        self.font_lg = pygame.font.SysFont("Arial", 22, bold=True)
        self.font_xl = pygame.font.SysFont("Arial", 32, bold=True)
        self.font_huge = pygame.font.SysFont("Arial", 120, bold=True)

        self.have_music = True
        try:
            pygame.mixer.music.load(self.audio_path)
            snd = pygame.mixer.Sound(self.audio_path)
            self.duration = snd.get_length()
            del snd
        except Exception as e:
            print(f"[경고] 오디오 로드 실패: {e} — 무음 상태로 녹음합니다.")
            self.have_music = False
            self.duration = 600.0

    # ── 재생 / 시간 ──────────────────────────────────────
    def start_playback(self):
        if self.have_music:
            try:
                pygame.mixer.music.play(start=self.song_time)
            except Exception:
                pygame.mixer.music.play()
        self.wall_ref = pygame.time.get_ticks()
        self.time_ref = self.song_time
        self.paused   = False
        self.state    = "play"

    def get_song_time(self):
        if self.state != "play" or self.paused:
            return self.song_time
        elapsed = (pygame.time.get_ticks() - self.wall_ref) / 1000.0
        return self.time_ref + elapsed

    def toggle_pause(self, now):
        if self.state != "play":
            return
        if self.paused:
            if self.have_music:
                pygame.mixer.music.unpause()
            self.wall_ref = now
            self.time_ref = self.song_time
            self.paused = False
        else:
            self.song_time = self.get_song_time()
            if self.have_music:
                pygame.mixer.music.pause()
            self.paused = True

    def seek(self, delta):
        self.song_time = max(0.0, min(self.get_song_time() + delta, self.duration))
        self.markers.clear()
        if self.have_music and not self.paused:
            try:
                pygame.mixer.music.play(start=self.song_time)
            except Exception:
                pygame.mixer.music.play()
            self.wall_ref = pygame.time.get_ticks()
            self.time_ref = self.song_time

    # ── 기록 ─────────────────────────────────────────────
    def record(self, lane):
        """현재 재생 시각에 노트를 기록(지연 보정 반영)."""
        if self.state != "play" or self.paused:
            return
        t = max(0.0, self.get_song_time() - self.offset)
        self.recorded.append({"time": round(float(t), 4), "lane": lane})
        self.flash[lane - 1] = FLASH_MS
        self.markers.append({"lane": lane, "y": float(JUDGE_Y), "alpha": 255.0})

    def undo(self):
        if self.recorded:
            self.recorded.pop()
        if self.markers:
            self.markers.pop()

    # ── 저장 ─────────────────────────────────────────────
    def save(self):
        notes = sorted(self.recorded, key=lambda n: (n["time"], n["lane"]))
        data = {
            "source":     os.path.basename(self.audio_path),
            "difficulty": f"{self.lane_count}k-{self.difficulty}",
            "meta": {
                "bpm":              round(float(self.bpm), 4),
                "seconds_per_beat": round(float(self.approach), 4),
            },
            "notes": notes,
        }
        with open(self.output_path, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
        print(f"저장 완료: {self.output_path}  ({len(notes)}노트)")

    # ── 업데이트(피드백 마커) ────────────────────────────
    def update_markers(self, dt):
        for mk in self.markers:
            mk["y"]     -= MARKER_SPEED * dt
            mk["alpha"] -= 255.0 * dt / 1.2        # 약 1.2초에 걸쳐 사라짐
        self.markers = [m for m in self.markers if m["alpha"] > 0 and m["y"] > -NOTE_H]
        self.flash = [max(0, v - dt * 1000) for v in self.flash]

    # ── 그리기 ───────────────────────────────────────────
    def draw(self, song_time):
        self.screen.fill(BG)

        # 레인 배경
        for i in range(self.lane_count):
            cx = self.lane_x[i]
            rect = pygame.Rect(cx - NOTE_W // 2 - 2, 0, NOTE_W + 4, H)
            pygame.draw.rect(self.screen, LANE_BG, rect)
            pygame.draw.rect(self.screen, LANE_BORDER, rect, 1)

        # 왼손/오른손 구분선
        if 0 < self.half < self.lane_count:
            mid_x = (self.lane_x[self.half - 1] + self.lane_x[self.half]) // 2
            pygame.draw.line(self.screen, DIVIDER, (mid_x, 0), (mid_x, JUDGE_Y + 10), 2)

        # 기록된 노트(위로 떠오르며 페이드) — 방금 무엇을 쳤는지 확인용
        for mk in self.markers:
            li = mk["lane"] - 1
            cx = self.lane_x[li]
            y  = int(mk["y"])
            a  = int(max(0, min(255, mk["alpha"])))
            surf = pygame.Surface((NOTE_W, NOTE_H), pygame.SRCALPHA)
            pygame.draw.rect(surf, (*self.colors[li], a), surf.get_rect(), border_radius=5)
            self.screen.blit(surf, (cx - NOTE_W // 2, y - NOTE_H // 2))

        # 판정선
        pygame.draw.line(self.screen, JUDGE_LINE,
                         (LANE_MARGIN - 10, JUDGE_Y), (W - LANE_MARGIN + 10, JUDGE_Y), 2)

        # 판정 버튼 & 입력 플래시
        for i in range(self.lane_count):
            cx = self.lane_x[i]
            color = self.colors[i]
            btn_rect = pygame.Rect(cx - NOTE_W // 2, JUDGE_Y - NOTE_H // 2, NOTE_W, NOTE_H)
            base_c = tuple(int(c * 0.35) for c in color)
            draw_rounded_rect(self.screen, base_c, btn_rect, radius=5)
            alpha = min(1.0, self.flash[i] / FLASH_MS)
            if alpha > 0:
                draw_glow(self.screen, color, btn_rect)
                flash_surf = pygame.Surface((NOTE_W, NOTE_H), pygame.SRCALPHA)
                pygame.draw.rect(flash_surf, (*color, int(220 * alpha)),
                                 flash_surf.get_rect(), border_radius=5)
                self.screen.blit(flash_surf, btn_rect.topleft)

        # 레인 라벨: 번호 + 키
        for i in range(self.lane_count):
            cx = self.lane_x[i]
            t1 = self.font_md.render(str(i + 1), True, self.colors[i])
            t2 = self.font_lg.render(self.hints[i], True, (210, 210, 230))
            self.screen.blit(t1, t1.get_rect(centerx=cx, centery=JUDGE_Y + 34))
            self.screen.blit(t2, t2.get_rect(centerx=cx, centery=JUDGE_Y + 58))

        self._draw_hud(song_time)

        # 카운트인 오버레이
        if self.state == "countin":
            remain = self.countin - (pygame.time.get_ticks() - self.countin_start) / 1000.0
            num = max(1, int(remain) + 1)
            t = self.font_huge.render(str(num), True, (255, 220, 120))
            self.screen.blit(t, t.get_rect(center=(W // 2, H // 2 - 20)))
            g = self.font_md.render("준비… 곧 시작합니다", True, (180, 180, 200))
            self.screen.blit(g, g.get_rect(center=(W // 2, H // 2 + 60)))

    def _draw_hud(self, song_time):
        # 진행 바
        bar_x, bar_y = 60, H - 28
        bar_w, bar_h = W - 120, 8
        pygame.draw.rect(self.screen, (50, 50, 70), (bar_x, bar_y, bar_w, bar_h), border_radius=4)
        if self.duration > 0:
            fill_w = int(bar_w * min(1.0, song_time / self.duration))
            if fill_w > 0:
                pygame.draw.rect(self.screen, (120, 160, 255),
                                 (bar_x, bar_y, fill_w, bar_h), border_radius=4)

        time_str = f"{fmt_time(song_time)} / {fmt_time(self.duration)}"
        self.screen.blit(self.font_md.render(time_str, True, (200, 200, 220)), (bar_x, H - 52))
        off_str = f"offset {self.offset*1000:.0f}ms"
        t_off = self.font_sm.render(off_str, True, (150, 150, 180))
        self.screen.blit(t_off, (W - 60 - t_off.get_width(), H - 50))

        # 상단: 모드 + 기록 수
        head = f"{self.lane_count}K REC   기록 {len(self.recorded)}개"
        t_head = self.font_md.render(head, True, (160, 180, 220))
        self.screen.blit(t_head, (W // 2 - t_head.get_width() // 2, 14))

        # 그룹 라벨
        if 0 < self.half < self.lane_count:
            self.screen.blit(self.font_sm.render("◀ 왼손", True, (100, 160, 255)),
                             (LANE_MARGIN, 14))
            t_r = self.font_sm.render("오른손 ▶", True, (255, 160, 100))
            self.screen.blit(t_r, (W - LANE_MARGIN - t_r.get_width(), 14))

        # 조작 힌트
        hint = "레인키: 기록  |  SPACE: 정지  |  Z: 취소  |  ← →: ±5초  |  ENTER: 저장  |  Q: 저장&종료"
        t_hint = self.font_sm.render(hint, True, (90, 90, 110))
        self.screen.blit(t_hint, t_hint.get_rect(centerx=W // 2, centery=H - 12))

        if self.paused:
            t_pause = self.font_xl.render("PAUSED", True, (255, 200, 80))
            self.screen.blit(t_pause, t_pause.get_rect(center=(W // 2, H // 2)))

    # ── 메인 루프 ────────────────────────────────────────
    def run(self):
        self.init_pygame()
        if self.state == "countin":
            self.countin_start = pygame.time.get_ticks()
        else:
            self.start_playback()

        while True:
            dt  = self.clock.tick(FPS) / 1000.0
            now = pygame.time.get_ticks()

            # 카운트인 종료 → 재생 시작
            if self.state == "countin" and (now - self.countin_start) / 1000.0 >= self.countin:
                self.start_playback()

            for event in pygame.event.get():
                if event.type == pygame.QUIT:
                    self.save(); pygame.quit(); sys.exit()
                if event.type != pygame.KEYDOWN:
                    continue

                key = event.key
                if key in (pygame.K_ESCAPE, pygame.K_q):
                    self.save(); pygame.quit(); sys.exit()
                elif key == pygame.K_SPACE:
                    self.toggle_pause(now)
                elif key in (pygame.K_z, pygame.K_BACKSPACE):
                    self.undo()
                elif key == pygame.K_LEFT:
                    self.seek(-5.0)
                elif key == pygame.K_RIGHT:
                    self.seek(+5.0)
                elif key == pygame.K_RETURN:
                    self.save()
                elif key in self.key_to_lane:
                    self.record(self.key_to_lane[key])

            song_time = self.get_song_time()
            if self.state == "play" and not self.paused:
                self.song_time = song_time
            self.update_markers(dt)

            self.draw(song_time)
            pygame.display.flip()

            # 곡이 끝나면 자동 저장 후 종료
            if self.state == "play" and not self.paused \
                    and self.have_music and song_time >= self.duration + 0.5:
                self.save(); pygame.quit(); sys.exit()


# ══════════════════════════════════════════════════════════
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="채보 수동 입력 레코더 (2/4key)")
    parser.add_argument("audio")
    parser.add_argument("--keys", type=int, choices=[2, 4], required=True)
    parser.add_argument("--offset", type=float, default=0.0,
                        help="입력 지연 보정(ms). 양수면 기록 시각을 그만큼 앞당김. 기본 0")
    parser.add_argument("--approach", type=float, default=0.8,
                        help="meta.seconds_per_beat 값(게임 previewWindow). 기본 0.8")
    parser.add_argument("--bpm", type=float, default=0.0,
                        help="meta.bpm 값. 미지정 시 0")
    parser.add_argument("--difficulty", default="manual",
                        help="difficulty 라벨. 결과는 '<keys>k-<라벨>'. 기본 manual")
    parser.add_argument("--countin", type=float, default=3.0,
                        help="시작 전 카운트다운(초). 0이면 즉시 시작. 기본 3")
    parser.add_argument("--output", default=None)
    args = parser.parse_args()

    if not os.path.exists(args.audio):
        sys.exit(f"오디오 파일 없음: {args.audio}")

    output = args.output or (args.audio.rsplit(".", 1)[0] + f"_{args.keys}k_manual.json")

    print(f"[레코더] {args.keys}key  키: {' '.join(KEY_LAYOUT[args.keys])}")
    print(f"  저장 위치: {output}")
    Recorder(args.audio, args.keys, args.offset, args.approach, args.bpm,
             args.difficulty, args.countin, output).run()
