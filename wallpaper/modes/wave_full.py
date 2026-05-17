from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
from typing import List
import math

class WaveFullMode(ModeBase):
    # Full-screen tiled wave.
    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider
        self.phase = 0.0
        self.wave_speed = 2.0
        self.amplitude = 0.2
        self.base_texts = [
            ">>> FAKE TERMINAL WAVE MODE <<<",
            "--- wallpaper engine style loop ---",
        ]

    def update(self, dt: float, width: int, height: int, t_abs: float):
        cycles_per_second = self.wave_speed / 60.0
        self.phase += cycles_per_second * dt * self.speed_factor

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf: List[str] = []
        amp_chars = max(2, int(self.amplitude * width))
        for row in range(height):
            text = self.base_texts[row % len(self.base_texts)]
            chunk = text + "   "
            repeat_count = (width // len(chunk)) + 3
            base_line = chunk * repeat_count

            angle = 2 * math.pi * (self.phase + row / max(1, height))
            offset = int((math.sin(angle) + 1) / 2 * amp_chars)

            start = offset % len(base_line)
            segment = base_line[start:start+width]
            if len(segment) < width:
                segment += base_line[: width - len(segment)]
            col = self.color_provider.get(t_abs, row)
            buf.append(col + segment[:width] + RESET)
        return "\n".join(buf)