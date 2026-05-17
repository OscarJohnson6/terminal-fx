from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
from typing import Optional, List, Dict
import random

class MatrixMode(ModeBase):
    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider
        self.columns: List[Optional[Dict]] = []
        self.density = 0.05

    def update(self, dt: float, width: int, height: int, t_abs: float):
        if len(self.columns) != width:
            self.columns = [None] * width
        for x in range(width):
            drop = self.columns[x]
            if drop is None:
                if random.random() < self.density * self.speed_factor:
                    self.columns[x] = {"y": 0.0, "speed": random.uniform(12, 35) * self.speed_factor}
            else:
                drop["y"] += drop["speed"] * dt
                if drop["y"] > height + random.randint(0, height):
                    self.columns[x] = None

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = [[" " for _ in range(width)] for _ in range(height)]
        for x, drop in enumerate(self.columns):
            if drop is None:
                continue
            head_y = int(drop["y"])
            head_color = self.color_provider.get(t_abs, x)
            tail_color = "\033[38;5;236m"
            for i in range(6):
                y = head_y - i
                if 0 <= y < height:
                    ch = random.choice("01abcdef")
                    if i == 0:
                        buf[y][x] = head_color + ch + RESET
                    else:
                        buf[y][x] = tail_color + ch + RESET
        return "\n".join("".join(row) for row in buf)