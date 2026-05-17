from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET, DIM, BRIGHT
from typing import Optional, List
import random
import math


class PendulumMode(ModeBase):
    """
    Single pendulum with a trailing bob.
    - String is attached at the top-center of the terminal.
    - Bob is a multi-line ASCII ball (extra-small / small / large).
    """

    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider

        # using a double-pendulum system just for nice chaotic motion
        self.theta1 = math.pi / 2
        self.theta2 = math.pi / 2
        self.omega1 = 0.0
        self.omega2 = 0.0

        self.g = 9.8
        self.len1 = 1.0
        self.len2 = 1.0
        self.damping = 0.0001  # tiny damping

        self.last_dims: Optional[tuple[int, int]] = None

        # trail of bob positions (center of ball)
        self.trail: List[tuple[int, int]] = []
        self.max_trail_len = 200

        # color seeds
        self.seed_rod = random.randint(0, 10000)
        self.seed_trail = random.randint(0, 10000)

    def _reset_for_size(self, width: int, height: int):
        self.last_dims = (width, height)
        total_len = max(4, int(height * 0.70))
        self.len1 = total_len * 0.5
        self.len2 = total_len * 0.5
        self.trail.clear()

    def _step_physics(self, dt: float):
        g = self.g * self.speed_factor

        m1 = 1.0
        m2 = 1.0
        L1 = self.len1
        L2 = self.len2
        t1 = self.theta1
        t2 = self.theta2
        w1 = self.omega1
        w2 = self.omega2

        denom = 2 * m1 + m2 - m2 * math.cos(2 * t1 - 2 * t2)
        if abs(denom) < 1e-6:
            denom = 1e-6

        num1 = -g * (2 * m1 + m2) * math.sin(t1)
        num2 = -m2 * g * math.sin(t1 - 2 * t2)
        num3 = -2 * math.sin(t1 - t2) * m2 * (w2 * w2 * L2 + w1 * w1 * L1 * math.cos(t1 - t2))
        alpha1 = (num1 + num2 + num3) / (L1 * denom)

        num4 = 2 * math.sin(t1 - t2)
        num5 = w1 * w1 * L1 * (m1 + m2)
        num6 = g * (m1 + m2) * math.cos(t1)
        num7 = w2 * w2 * L2 * m2 * math.cos(t1 - t2)
        alpha2 = num4 * (num5 + num6 + num7) / (L2 * denom)

        w1 += alpha1 * dt
        w2 += alpha2 * dt

        # damping
        w1 *= (1 - self.damping)
        w2 *= (1 - self.damping)

        # if it almost stops, give it a gentle random nudge
        if abs(w1) + abs(w2) < 0.01:
            w1 += (random.random() - 0.5) * 0.2
            w2 += (random.random() - 0.5) * 0.2

        t1 += w1 * dt
        t2 += w2 * dt

        self.theta1 = t1
        self.theta2 = t2
        self.omega1 = w1
        self.omega2 = w2

    # ---------------- size / pattern helpers ----------------

    def _choose_ball_pattern(self, width: int, height: int):
        """
        Choose extra-small, small, or large ball pattern based on terminal area.
        Returns (pattern_list, height, width).
        """
        area = width * height

        # extra small: just a single 'O'
        if area < 800:
            pattern = ["O"]
        # small ball ( __ /  \ \__/ )
        elif area < 4000:
            pattern = [
                " __ ",
                "/  \\",
                "\\__/",
            ]
        # large ball for big terminals
        else:
            pattern = [
                "  ____  ",
                " /    \\ ",
                "|      |",
                " \\____/ ",
            ]

        h = len(pattern)
        w = len(pattern[0]) if h > 0 else 0
        return pattern, h, w

    def _draw_ball_centered(self, buf, cx: int, cy: int, col: str, width: int, height: int):
        """
        Draw the ball centered at (cx, cy) using the chosen pattern.
        """
        pattern, ph, pw = self._choose_ball_pattern(width, height)

        top_y = cy - ph // 2
        left_x = cx - pw // 2

        for row_idx, row in enumerate(pattern):
            y = top_y + row_idx
            if not (0 <= y < height):
                continue
            for col_idx, ch in enumerate(row):
                x = left_x + col_idx
                if 0 <= x < width and ch != " ":
                    buf[y][x] = col + ch + RESET

        # return top/left so caller can know ball bounds
        return top_y, ph, pw

    # ---------------- main update / render ----------------

    def update(self, dt: float, width: int, height: int, t_abs: float):
        if width < 10 or height < 10:
            return

        if self.last_dims != (width, height):
            self._reset_for_size(width, height)

        self._step_physics(dt * self.speed_factor)

        # pivot at top center
        cx = width // 2
        cy = 1

        # first joint
        x1 = cx + int(self.len1 * math.sin(self.theta1))
        y1 = cy + int(self.len1 * math.cos(self.theta1))

        # bob center
        x2 = x1 + int(self.len2 * math.sin(self.theta2))
        y2 = y1 + int(self.len2 * math.cos(self.theta2))

        x2 = max(0, min(width - 1, x2))
        y2 = max(0, min(height - 1, y2))

        # trail stores the center of the ball
        self.trail.append((x2, y2))
        if len(self.trail) > self.max_trail_len:
            self.trail = self.trail[-self.max_trail_len:]

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = [[" " for _ in range(width)] for _ in range(height)]

        if width < 10 or height < 10:
            return "\n".join("".join(row) for row in buf)

        cx = width // 2
        cy = 1  # pivot

        # recompute bob center from angles
        x1 = cx + int(self.len1 * math.sin(self.theta1))
        y1 = cy + int(self.len1 * math.cos(self.theta1))
        x2 = x1 + int(self.len2 * math.sin(self.theta2))
        y2 = y1 + int(self.len2 * math.cos(self.theta2))

        x1 = max(0, min(width - 1, x1))
        y1 = max(0, min(height - 1, y1))
        x2 = max(0, min(width - 1, x2))
        y2 = max(0, min(height - 1, y2))

        # faster color cycling for this mode
        color_time = t_abs * 2.0

        # decide ball size and bounding box around the bob center
        pattern, ph, pw = self._choose_ball_pattern(width, height)
        top_y = y2 - ph // 2
        left_x = x2 - pw // 2
        bottom_y = top_y + ph - 1
        right_x = left_x + pw - 1

        # draw trail (skip inside the ball)
        col_trail = self.color_provider.get(color_time, self.seed_trail)
        fade_len = max(1, len(self.trail))
        for i, (tx, ty) in enumerate(self.trail):
            if 0 <= tx < width and 0 <= ty < height:
                if left_x <= tx <= right_x and top_y <= ty <= bottom_y:
                    continue
                col = DIM + col_trail if i < fade_len * 0.5 else col_trail
                buf[ty][tx] = col + "." + RESET

        # rod color
        col_rod = self.color_provider.get(color_time, self.seed_rod)

        # draw ball (so we know the actual top_y)
        bob_col = BRIGHT + col_rod
        top_y, ph, pw = self._draw_ball_centered(buf, x2, y2, bob_col, width, height)

        # string anchor: top middle of the ball
        anchor_y = max(1, top_y)
        anchor_x = x2

        def draw_line(xa, ya, xb, yb):
            dx = xb - xa
            dy = yb - ya
            steps = max(abs(dx), abs(dy))
            if steps == 0:
                return

            if abs(dy) >= abs(dx) * 2:
                ch = "|"
            elif abs(dx) >= abs(dy) * 2:
                ch = "-"
            else:
                if (dx > 0 and dy > 0) or (dx < 0 and dy < 0):
                    ch = "\\"
                else:
                    ch = "/"

            for s in range(steps + 1):
                t = s / steps
                x = int(round(xa + dx * t))
                y = int(round(ya + dy * t))
                if 0 <= x < width and 0 <= y < height:
                    if y <= anchor_y:  # don't draw over the ball
                        buf[y][x] = col_rod + ch + RESET

        # string from pivot to top of ball
        draw_line(cx, cy, anchor_x, anchor_y)

        # tiny connector at top of the ball (_| or |_)
        dx_anchor = anchor_x - cx
        if dx_anchor < -1:
            # ball is left of center => "|_"
            if 0 <= anchor_x < width:
                buf[anchor_y][anchor_x] = col_rod + "|" + RESET
            if anchor_x + 1 < width:
                buf[anchor_y][anchor_x + 1] = col_rod + "_" + RESET
        elif dx_anchor > 1:
            # ball is right of center => "_|"
            if anchor_x - 1 >= 0:
                buf[anchor_y][anchor_x - 1] = col_rod + "_" + RESET
            if 0 <= anchor_x < width:
                buf[anchor_y][anchor_x] = col_rod + "|" + RESET
        else:
            # near center – just a '|'
            if 0 <= anchor_x < width:
                buf[anchor_y][anchor_x] = col_rod + "|" + RESET

        return "\n".join("".join(row) for row in buf)
