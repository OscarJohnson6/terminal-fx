from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
from typing import List, Dict
import random, math

class BurstsMode(ModeBase):
    # Random explosion bursts across the screen (no rockets).
    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider
        self.particles: List[Dict] = []
        self.time_since_burst = 0.0
        self.base_interval = 0.8

        # Smoothing / physics tuning
        self.gravity = 12.0      # px/s^2 downward
        self.drag = 0.90         # velocity multiplier per second (approx)
        self.max_substeps = 3    # small substepping for smoother motion

    def spawn_burst(self, width: int, height: int, num: int = 40):
        # Center-ish, but random so it feels alive
        x = random.uniform(5, max(6, width - 5))
        y = random.uniform(3, max(4, height - 4))
        seed = random.randint(0, 10000)

        for _ in range(num):
            angle = random.uniform(0, 2 * math.pi)
            speed = random.uniform(6, 20) * self.speed_factor

            vx = speed * math.cos(angle)
            # Slightly squashed vertically for a “fan” feel
            vy = speed * math.sin(angle) * 0.7

            max_life = random.uniform(0.8, 1.6)
            # Choose a fixed character per particle for less flicker
            ch = random.choice("*+x.o")

            self.particles.append({
                "x": x,
                "y": y,
                "vx": vx,
                "vy": vy,
                "life": max_life,   # remaining life
                "max_life": max_life,
                "seed": seed,
                "char": ch,
            })

    def update(self, dt: float, width: int, height: int, t_abs: float):
        # Spawn new bursts at a rate adjusted by speed_factor
        self.time_since_burst += dt
        interval = self.base_interval / max(0.1, self.speed_factor)
        if self.time_since_burst >= interval:
            self.time_since_burst = 0.0
            self.spawn_burst(width, height)

        # Substep the physics for smoother motion when dt is large
        # (e.g., low frame rate)
        substeps = max(1, min(self.max_substeps, int(dt * 60)))
        sub_dt = dt / substeps

        new_parts: List[Dict] = []
        for p in self.particles:
            # age only once per frame, so life isn't tied to substeps
            p["life"] -= dt
            if p["life"] <= 0:
                continue

            vx, vy = p["vx"], p["vy"]

            for _ in range(substeps):
                # Apply drag (exponential-ish over time)
                drag_factor = self.drag ** sub_dt
                vx *= drag_factor
                vy *= drag_factor

                # Gravity pulls downward for a nice arc
                vy += self.gravity * sub_dt

                p["x"] += vx * sub_dt
                p["y"] += vy * sub_dt

            p["vx"], p["vy"] = vx, vy
            new_parts.append(p)

        self.particles = new_parts

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = [[" " for _ in range(width)] for _ in range(height)]

        for p in self.particles:
            # Life-based intensity: early = solid, late = sparse/faint
            life_ratio = max(0.0, min(1.0, p["life"] / p["max_life"]))
            # Drop older particles occasionally so they fade instead of hard-pop
            if life_ratio < 0.2 and random.random() < 0.5:
                continue

            x = int(p["x"])
            y = int(p["y"])
            if 0 <= x < width and 0 <= y < height:
                col = self.color_provider.get(t_abs, p["seed"])

                ch = p["char"]
                # As it ages, use a “lighter” symbol
                if life_ratio < 0.4 and ch == "*":
                    ch = "."

                buf[y][x] = col + ch + RESET

        return "\n".join("".join(row) for row in buf)
