from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
from typing import List, Dict, Optional
import math, random


class OrbitalsMode(ModeBase):
    """
    Particles orbiting around a few smoothly moving attractors.
    Looks like glowing electrons / orbitals.
    """

    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider

        self.attractors: List[Dict] = []  # {phase_x, phase_y, rx, ry, wx, wy, seed}
        self.orbiters: List[Dict] = []    # {x, y, vx, vy, anchor_idx, seed}
        self.last_dims: Optional[tuple[int, int]] = None

        self.num_attractors = 3
        self.num_orbiters_per_attractor = 40

        # spring-like attraction parameters
        self.k = 12.0        # spring strength
        self.damping = 4.0   # velocity damping
        self.max_speed = 60.0

    def _init_system(self, width: int, height: int):
        self.attractors.clear()
        self.orbiters.clear()
        self.last_dims = (width, height)

        cx = width / 2.0
        cy = height / 2.0
        base_r = min(width, height) * 0.25

        # define a few attractors that move on lissajous-ish paths
        for i in range(self.num_attractors):
            phase_x = random.uniform(0, 2 * math.pi)
            phase_y = random.uniform(0, 2 * math.pi)

            rx = base_r * random.uniform(0.6, 1.1)
            ry = base_r * random.uniform(0.4, 0.9)

            wx = random.uniform(0.15, 0.35) * (1.0 + 0.3 * i)
            wy = random.uniform(0.12, 0.30) * (1.0 + 0.2 * i)

            seed = random.randint(0, 100000)

            self.attractors.append({
                "cx": cx,
                "cy": cy,
                "phase_x": phase_x,
                "phase_y": phase_y,
                "rx": rx,
                "ry": ry,
                "wx": wx,
                "wy": wy,
                "seed": seed,
            })

        # orbiters around each attractor
        for idx, att in enumerate(self.attractors):
            for _ in range(self.num_orbiters_per_attractor):
                angle = random.uniform(0, 2 * math.pi)
                dist = random.uniform(2, base_r * 0.8)
                x = att["cx"] + dist * math.cos(angle)
                y = att["cy"] + dist * math.sin(angle)
                seed = random.randint(0, 1000000)

                self.orbiters.append({
                    "x": x,
                    "y": y,
                    "vx": 0.0,
                    "vy": 0.0,
                    "anchor_idx": idx,
                    "seed": seed,
                })

    def _attractor_position(self, att: Dict, t: float):
        # smooth lissajous-like motion
        ax = att["cx"] + att["rx"] * math.sin(att["wx"] * t + att["phase_x"])
        ay = att["cy"] + att["ry"] * math.sin(att["wy"] * t + att["phase_y"])
        return ax, ay

    def update(self, dt: float, width: int, height: int, t_abs: float):
        if width < 5 or height < 5:
            return

        dims = (width, height)
        if self.last_dims != dims or not self.attractors or not self.orbiters:
            self._init_system(width, height)

        # effective speed scaling
        dt_scaled = dt * max(0.3, self.speed_factor)

        # update orbiters
        for orb in self.orbiters:
            idx = orb["anchor_idx"] % len(self.attractors)
            att = self.attractors[idx]

            ax, ay = self._attractor_position(att, t_abs * 0.8)

            dx = ax - orb["x"]
            dy = ay - orb["y"]

            # spring-like acceleration toward attractor minus damping
            ax_force = self.k * dx - self.damping * orb["vx"]
            ay_force = self.k * dy - self.damping * orb["vy"]

            orb["vx"] += ax_force * dt_scaled
            orb["vy"] += ay_force * dt_scaled

            # clamp max speed
            speed = math.hypot(orb["vx"], orb["vy"])
            if speed > self.max_speed:
                scale = self.max_speed / max(speed, 1e-6)
                orb["vx"] *= scale
                orb["vy"] *= scale

            orb["x"] += orb["vx"] * dt_scaled
            orb["y"] += orb["vy"] * dt_scaled

            # gentle wraparound instead of hard clipping
            if orb["x"] < 0:
                orb["x"] += width
            elif orb["x"] >= width:
                orb["x"] -= width
            if orb["y"] < 0:
                orb["y"] += height
            elif orb["y"] >= height:
                orb["y"] -= height

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = [[" " for _ in range(width)] for _ in range(height)]

        # draw orbiters
        for orb in self.orbiters:
            x = int(orb["x"])
            y = int(orb["y"])
            if not (0 <= x < width and 0 <= y < height):
                continue

            speed = math.hypot(orb["vx"], orb["vy"])
            if speed < 8:
                ch = "."
            elif speed < 20:
                ch = "o"
            else:
                ch = "O"

            col = self.color_provider.get(t_abs, orb["seed"])
            buf[y][x] = col + ch + RESET

        # draw attractors as bright cores
        for att in self.attractors:
            ax, ay = self._attractor_position(att, t_abs * 0.8)
            ix = int(ax)
            iy = int(ay)
            if 0 <= ix < width and 0 <= iy < height:
                col = self.color_provider.get(t_abs, att["seed"])
                buf[iy][ix] = col + "@" + RESET

        return "\n".join("".join(row) for row in buf)
