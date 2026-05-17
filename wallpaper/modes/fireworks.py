from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
from typing import List, Dict
import math, random

DIM = "\033[2m"

class FireworksMode(ModeBase):
    # Vertical rockets that explode into radial bursts.
    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider
        self.rockets: List[Dict] = []
        self.particles: List[Dict] = []
        self.target_rockets = 4
        self.min_spacing = 10  # min columns between rockets to reduce overlap

    def spawn_rocket(self, width: int, height: int):
        # Try to keep rockets spaced out horizontally to reduce overlapping bursts.
        attempts = 0
        while True:
            x = random.uniform(2, max(3, width - 3))
            if all(abs(x - r["x"]) > self.min_spacing for r in self.rockets) or attempts > 20:
                break
            attempts += 1
        y = height + random.uniform(1, 5)
        vy = -random.uniform(20, 35) * self.speed_factor
        peak_y = random.uniform(height * 0.25, height * 0.6)
        trail_len = random.randint(3, 7)
        seed = random.randint(0, 10000)  # rocket + explosion share a color seed
        self.rockets.append({
            "x": x,
            "y": y,
            "vy": vy,
            "trail": trail_len,
            "peak_y": peak_y,
            "seed": seed,
        })

    def spawn_explosion(self, rocket: Dict, num: int = 40):
        x = rocket["x"]
        y = rocket["y"]
        seed = rocket["seed"]
        for _ in range(num):
            angle = random.uniform(0, 2 * math.pi)
            speed = random.uniform(8, 25) * self.speed_factor
            vx = speed * math.cos(angle)
            vy = speed * math.sin(angle) * 0.7
            life = random.uniform(0.4, 1.2)
            self.particles.append({
                "x": x,
                "y": y,
                "vx": vx,
                "vy": vy,
                "life": life,
                "seed": seed,
            })

    def update(self, dt: float, width: int, height: int, t_abs: float):
        while len(self.rockets) < self.target_rockets:
            self.spawn_rocket(width, height)

        new_rockets: List[Dict] = []
        for r in self.rockets:
            r["y"] += r["vy"] * dt
            if r["y"] <= r["peak_y"]:
                self.spawn_explosion(r)
                continue
            if r["y"] < -10:
                continue
            new_rockets.append(r)
        self.rockets = new_rockets

        new_parts: List[Dict] = []
        for p in self.particles:
            p["x"] += p["vx"] * dt
            p["y"] += p["vy"] * dt
            p["life"] -= dt
            if p["life"] > 0:
                new_parts.append(p)
        self.particles = new_parts

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = [[" " for _ in range(width)] for _ in range(height)]
        # rockets: bright head, dim trail (same color but "half opacity" via DIM)
        for r in self.rockets:
            head_x = int(r["x"])
            head_y = int(r["y"])
            trail = r["trail"]
            head_col = self.color_provider.get(t_abs, r["seed"])
            for i in range(trail):
                yy = head_y + i
                if 0 <= head_x < width and 0 <= yy < height:
                    if i == 0:
                        ch = "|"
                        col = head_col
                    else:
                        ch = "|" if i < trail - 1 else "."
                        col = DIM + head_col  # darker version of the head color
                    buf[yy][head_x] = col + ch + RESET
        # particles: unified color per explosion (less visual chaos)
        for p in self.particles:
            x = int(p["x"])
            y = int(p["y"])
            if 0 <= x < width and 0 <= y < height:
                col = self.color_provider.get(t_abs, p["seed"])
                ch = random.choice("*+x0123456789abcdef")
                buf[y][x] = col + ch + RESET
        return "\n".join("".join(row) for row in buf)