from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
import math

class PulseMode(ModeBase):
    """
    Expanding concentric rings from the center.
    Feels like energy waves / breathing. Good calm-but-energized background.
    """
    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider

        self.t = 0.0

    def update(self, dt: float, width: int, height: int, t_abs: float):
        self.t += dt * self.speed_factor

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = [[" " for _ in range(width)] for _ in range(height)]

        cx = width / 2.0
        cy = height / 2.0
        max_r = math.hypot(cx, cy)

        # base phase
        phase = self.t

        # draw rings at several phase offsets
        num_rings = 4
        for k in range(num_rings):
            # 0..1 offset per ring
            offset = k / num_rings
            # base radius for this ring as a function of time
            r_norm = (phase * 0.5 + offset) % 1.0
            radius = 2 + r_norm * (max_r * 0.8)

            # thickness of ring in pixels
            thickness = 1.3

            col = self.color_provider.get(t_abs, k * 1234)

            # sample around the circle
            steps = int(2 * math.pi * radius * 0.5)  # fewer points than full circumference
            steps = max(12, min(steps, 600))
            for i in range(steps):
                ang = 2 * math.pi * i / steps
                x = cx + radius * math.cos(ang)
                y = cy + radius * math.sin(ang)
                ix = int(round(x))
                iy = int(round(y))
                if 0 <= ix < width and 0 <= iy < height:
                    buf[iy][ix] = col + "o" + RESET

        return "\n".join("".join(row) for row in buf)