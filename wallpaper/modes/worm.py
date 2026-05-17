from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
import random, math
from typing import List, Dict


class WormFieldMode(ModeBase):
    """
    Smooth 'flow field' worm-trails drifting around the screen.
    Calming, mesmerizing, minimal-physics ASCII simulation.
    """

    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider
        self.worms: List[Dict] = []
        self.last_dims = None
        self.num_worms = 35

    def _init_worms(self, width, height):
        self.worms.clear()
        for _ in range(self.num_worms):
            ang = random.uniform(0, 2 * math.pi)
            speed = random.uniform(4, 12) * self.speed_factor
            self.worms.append({
                "x": random.uniform(0, width),
                "y": random.uniform(0, height),
                "vx": math.cos(ang) * speed,
                "vy": math.sin(ang) * speed,
                "seed": random.randint(0, 999999),
                "phase": random.uniform(0, 1000),
            })

    def update(self, dt: float, width: int, height: int, t_abs: float):
        if (width, height) != self.last_dims or not self.worms:
            self.last_dims = (width, height)
            if width > 5 and height > 5:
                self._init_worms(width, height)
            else:
                return

        # Global flow rotation
        flow_angle = math.sin(t_abs * 0.2) * 0.8

        for worm in self.worms:
            # Worm’s personal phase drift
            worm["phase"] += dt * 0.8

            # Compute flow direction from phase + global flow
            dir_angle = flow_angle + math.sin(worm["phase"] * 0.7) * 1.3
            speed = 10 * self.speed_factor

            # Smooth steering toward the direction
            worm["vx"] += math.cos(dir_angle) * speed * dt
            worm["vy"] += math.sin(dir_angle) * speed * dt

            # Soft velocity damping to prevent runaway speeds
            worm["vx"] *= 0.96
            worm["vy"] *= 0.96

            worm["x"] += worm["vx"] * dt
            worm["y"] += worm["vy"] * dt

            # Soft boundaries (not bouncing, just steering inward)
            margin = 3
            if worm["x"] < margin:
                worm["vx"] += (margin - worm["x"]) * dt * 20
            if worm["x"] > width - margin:
                worm["vx"] -= (worm["x"] - (width - margin)) * dt * 20
            if worm["y"] < margin:
                worm["vy"] += (margin - worm["y"]) * dt * 20
            if worm["y"] > height - margin:
                worm["vy"] -= (worm["y"] - (height - margin)) * dt * 20

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = [[" " for _ in range(width)] for _ in range(height)]

        for worm in self.worms:
            x = int(worm["x"])
            y = int(worm["y"])
            if 0 <= x < width and 0 <= y < height:
                col = self.color_provider.get(t_abs, worm["seed"])
                
                # Density = speed/curvature
                speed = (worm["vx"]**2 + worm["vy"]**2)**0.5
                if speed < 4:
                    char = "·"
                elif speed < 8:
                    char = "o"
                elif speed < 12:
                    char = "O"
                else:
                    char = "@"

                buf[y][x] = col + char + RESET

        return "\n".join("".join(row) for row in buf)
