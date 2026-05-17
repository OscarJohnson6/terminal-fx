from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET
from typing import Optional, List, Dict
import random
import math


class BounceMode(ModeBase):
    """
    Gravity-driven bouncing balls with simple ball-ball collisions.
    Good 'screensaver' / workout background: simple physics, always moving.
    """
    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider

        self.balls: List[Dict] = []  # {x, y, vx, vy, radius, seed}
        self.last_dims: Optional[tuple[int, int]] = None

        # gravity and bounce tuning
        self.g = 20.0    # px/s^2 (scaled by speed_factor)

        # wall bounce: keep these close to 1 so they don't die out quickly
        self.restitution = 0.99      # energy retention on wall bounce
        self.friction = 0.995        # horizontal damping per floor/ceiling hit

        # ball-ball collision restitution (1.0 = perfectly elastic)
        self.collision_restitution = 0.99

        self.target_ball_count = 5

    def _init_balls(self, width: int, height: int):
        self.balls.clear()

        # Slightly bigger minimum radius so the glyph looks more “round”,
        # especially on small terminals.
        base_radius = max(2, min(width, height) // 20)

        cx = width / 2.0
        cy = height / 3.0
        ring_r = min(width, height) * 0.25

        for i in range(self.target_ball_count):
            # Place balls roughly on a circle instead of a random "triangle-ish" cluster
            angle = 2.0 * math.pi * (i / self.target_ball_count) + random.uniform(-0.2, 0.2)

            r = random.randint(max(1, base_radius - 1), base_radius + 1)
            x = cx + ring_r * math.cos(angle)
            y = cy + ring_r * math.sin(angle)

            # Keep inside bounds
            x = max(r + 1, min(width - r - 2, x))
            y = max(r + 1, min(height - r - 2, y))

            vx = random.uniform(-10, 10) * self.speed_factor
            vy = random.uniform(-5, 5) * self.speed_factor
            seed = random.randint(0, 10000)
            self.balls.append({
                "x": x,
                "y": y,
                "vx": vx,
                "vy": vy,
                "radius": r,
                "seed": seed,
            })

    def _handle_ball_collisions(self):
        """
        Simple elastic collisions between balls with equal-ish mass.
        This runs after wall collisions each frame.
        """
        n = len(self.balls)
        if n < 2:
            return

        for i in range(n):
            b1 = self.balls[i]
            x1, y1 = b1["x"], b1["y"]
            r1 = b1["radius"]

            for j in range(i + 1, n):
                b2 = self.balls[j]
                x2, y2 = b2["x"], b2["y"]
                r2 = b2["radius"]

                dx = x2 - x1
                dy = y2 - y1
                rs = r1 + r2
                dist_sq = dx * dx + dy * dy

                if dist_sq == 0 or dist_sq > rs * rs:
                    # No overlap / exactly same position (rare) -> skip
                    continue

                dist = math.sqrt(dist_sq) if dist_sq > 0 else rs * 0.999

                # Normal vector from b1 to b2
                nx = dx / dist
                ny = dy / dist

                # Push them apart so they don't stay overlapped
                overlap = rs - dist
                # Move each ball half the overlap along the normal
                b1["x"] -= nx * overlap * 0.5
                b1["y"] -= ny * overlap * 0.5
                b2["x"] += nx * overlap * 0.5
                b2["y"] += ny * overlap * 0.5

                # Relative velocity along normal
                vx1, vy1 = b1["vx"], b1["vy"]
                vx2, vy2 = b2["vx"], b2["vy"]
                rel_vn = (vx2 - vx1) * nx + (vy2 - vy1) * ny

                # If they are separating, no bounce needed
                if rel_vn > 0:
                    continue

                # Decompose into normal + tangential components
                # Tangent (perpendicular to normal)
                tx = -ny
                ty = nx

                v1n = vx1 * nx + vy1 * ny
                v1t = vx1 * tx + vy1 * ty
                v2n = vx2 * nx + vy2 * ny
                v2t = vx2 * tx + vy2 * ty

                # Equal-mass collision:
                # swap normal components, apply restitution factor
                e = self.collision_restitution
                v1n_after = v2n * e
                v2n_after = v1n * e

                # Tangential components stay the same
                v1t_after = v1t
                v2t_after = v2t

                # Recompose back to x/y
                b1["vx"] = v1n_after * nx + v1t_after * tx
                b1["vy"] = v1n_after * ny + v1t_after * ty
                b2["vx"] = v2n_after * nx + v2t_after * tx
                b2["vy"] = v2n_after * ny + v2t_after * ty

    def update(self, dt: float, width: int, height: int, t_abs: float):
        dims = (width, height)
        if dims != self.last_dims or not self.balls:
            self.last_dims = dims
            if width > 5 and height > 5:
                self._init_balls(width, height)
            else:
                return

        g = self.g * self.speed_factor

        for b in self.balls:
            r = b["radius"]

            # integrate velocity (gravity)
            b["vy"] += g * dt

            # integrate position
            b["x"] += b["vx"] * dt
            b["y"] += b["vy"] * dt

            # collisions with walls
            # floor / ceiling
            if b["y"] >= height - 1 - r:
                b["y"] = height - 1 - r
                b["vy"] = -b["vy"] * self.restitution
                b["vx"] *= self.friction

            elif b["y"] <= r:
                b["y"] = r
                b["vy"] = -b["vy"] * self.restitution
                b["vx"] *= self.friction

            # left / right walls
            if b["x"] <= r:
                b["x"] = r
                b["vx"] = -b["vx"] * self.restitution
            elif b["x"] >= width - 1 - r:
                b["x"] = width - 1 - r
                b["vx"] = -b["vx"] * self.restitution

        # After wall collisions, resolve ball-ball collisions
        self._handle_ball_collisions()

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = [[" " for _ in range(width)] for _ in range(height)]

        for b in self.balls:
            r = b["radius"]
            cx = int(round(b["x"]))
            cy = int(round(b["y"]))
            col = self.color_provider.get(t_abs, b["seed"])

            # Draw a filled "circle" in terminal space
            for dy in range(-r, r + 1):
                for dx in range(-r, r + 1):
                    # Basic circle fill
                    if dx * dx + dy * dy <= r * r + 0.5:
                        x = cx + dx
                        y = cy + dy
                        if 0 <= x < width and 0 <= y < height:
                            ch = "O" if dx == 0 and dy == 0 else "o"
                            buf[y][x] = col + ch + RESET

        return "\n".join("".join(row) for row in buf)
