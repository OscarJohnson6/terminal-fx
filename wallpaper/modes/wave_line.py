from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
from typing import List
import math

class WaveLineMode(ModeBase):
    # Single line of text that slithers across with whitespace around it.
    # This version only draws ONE copy of the text per row, so you don't get
    # the chopped ">>> SLI" fragments.
    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider
        self.phase = 0.0
        self.wave_speed = 1.0  # cycles per minute
        self.text = ">>> SLITHERING LINE <<<"

    def update(self, dt: float, width: int, height: int, t_abs: float):
        cycles_per_second = self.wave_speed / 60.0
        self.phase += cycles_per_second * dt * self.speed_factor

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf: List[str] = []
        text_segment = self.text
        seg_len = len(text_segment)
        if seg_len > width:
            text_segment = text_segment[:width]
            seg_len = width
        max_left = max(0, width - seg_len)

        for row in range(height):
            angle = 2 * math.pi * (self.phase + row / max(1, height))
            pos = (math.sin(angle) + 1) / 2  # 0..1
            left = int(pos * max_left)
            col = self.color_provider.get(t_abs, row)
            line = " " * left + col + text_segment + RESET
            buf.append(line[:width].ljust(width))
        return "\n".join(buf)
