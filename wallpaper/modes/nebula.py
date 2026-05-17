from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
from typing import Optional
import math


class NebulaMode(ModeBase):
    """
    Swirling nebula / plasma effect.
    Uses cheap analytic 'noise' built from sines,
    colored via the ColorProvider.
    """

    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider
        self.last_dims: Optional[tuple[int, int]] = None

    def update(self, dt: float, width: int, height: int, t_abs: float):
        # No persistent state required; all math done in render()
        self.last_dims = (width, height)

    def _sample_field(self, xn: float, yn: float, t: float) -> float:
        """
        xn, yn in [0,1]. Returns value in [0,1].
        This is a simple combination of a few sine 'waves'.
        """
        # multi-component 'plasma'
        v = 0.0
        v += math.sin(3.0 * xn + 0.7 * t)
        v += math.sin(2.0 * yn - 0.5 * t)
        v += math.sin(2.5 * xn + 4.0 * yn + 0.3 * t)
        v /= 3.0  # now roughly in [-1,1]
        return (v + 1.0) * 0.5  # -> [0,1]

    def render(self, width: int, height: int, t_abs: float) -> str:
        if width <= 0 or height <= 0:
            return ""

        # chars from dark -> bright
        chars = " .:-=+*#%@"
        n_chars = len(chars) - 1

        buf = [[" " for _ in range(width)] for _ in range(height)]

        # scale time by speed_factor so user can slow/speed the flow
        t = t_abs * max(0.2, self.speed_factor)

        for y in range(height):
            yn = y / max(1, height - 1)
            for x in range(width):
                xn = x / max(1, width - 1)

                v = self._sample_field(xn, yn, t)
                idx = int(v * n_chars)
                ch = chars[idx]

                # use a deterministic seed per cell so colors are spatially coherent
                seed = x * 73856093 ^ y * 19349663
                col = self.color_provider.get(t_abs, seed)

                buf[y][x] = col + ch + RESET

        return "\n".join("".join(row) for row in buf)
