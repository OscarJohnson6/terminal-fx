from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
from typing import List, Optional
import random


class FireMode(ModeBase):
    """
    ASCII campfire / torch-style fire using a simple fire CA.
    Heat rises from the bottom, diffuses upward, and cools.
    """

    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider

        self.last_dims: Optional[tuple[int, int]] = None
        self.heat: List[List[int]] = []  # heat[y][x] in [0,255]
        self.time_accum = 0.0
        self.step_dt = 1.0 / 30.0  # simulation step

    def _init_fire(self, width: int, height: int):
        self.last_dims = (width, height)
        self.heat = [[0 for _ in range(width)] for _ in range(height)]
        self.time_accum = 0.0

    def _step_fire(self, width: int, height: int):
        if height < 3 or width < 3:
            return

        # bottom row: inject random heat pulses
        bottom = height - 1
        for x in range(width):
            # base fire strength + flicker
            base = random.randint(180, 255)
            self.heat[bottom][x] = base

        # propagate upwards: each cell is an average of a few below it minus cooling
        for y in range(bottom - 1, -1, -1):  # from bottom-1 up to 0
            row = self.heat[y]
            row_below = self.heat[y + 1]
            # optionally also look 2 rows below
            row_below2 = self.heat[y + 2] if y + 2 < height else row_below

            for x in range(width):
                # sample neighbors in row below
                h = row_below[x]
                if x > 0:
                    h += row_below[x - 1]
                if x + 1 < width:
                    h += row_below[x + 1]
                # and a bit from two rows below for taller tongues
                h += row_below2[x]

                # average
                h //= 4

                # cooling (stronger as we go up)
                cooling = random.randint(0, 12 + y // 4)
                h = max(0, h - cooling)

                row[x] = h

    def update(self, dt: float, width: int, height: int, t_abs: float):
        if width < 5 or height < 5:
            return

        dims = (width, height)
        if self.last_dims != dims or not self.heat:
            self._init_fire(width, height)

        # Fixed-timestep simulation
        self.time_accum += dt * max(0.5, self.speed_factor)
        while self.time_accum >= self.step_dt:
            self.time_accum -= self.step_dt
            self._step_fire(width, height)

    def render(self, width: int, height: int, t_abs: float) -> str:
        if not self.heat:
            return ""

        # from coolest to hottest
        chars = " .:-=+*#%@"
        buf = [[" " for _ in range(width)] for _ in range(height)]

        for y in range(height):
            for x in range(width):
                h = self.heat[y][x]  # 0..255
                if h <= 0:
                    continue

                # normalize heat to [0,1]
                v = h / 255.0
                idx = int(v * (len(chars) - 1))
                ch = chars[idx]

                # use a warm-biased color mode via the ColorProvider
                # (your spectrum/static palettes will control the exact colors)
                seed = (y * 131 + x * 17) ^ 0xF00D
                col = self.color_provider.get(t_abs * 0.6, seed)

                buf[y][x] = col + ch + RESET

        return "\n".join("".join(row) for row in buf)
