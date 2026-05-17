from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET, DIM
from typing import Optional, List, Dict
import random, math

class OrbsMode(ModeBase):
    """
    Two orbiting 'balls' that spiral inward, collide, explode into pieces,
    then reform and repeat. Good 'physics-ish' loop for workout focus.
    """
    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider

        # State machine phases: 'orbit', 'converge', 'explode', 'reform'
        self.phase = "orbit"
        self.state_timer = 0.0

        self.theta = 0.0          # angular position for orbit
        self.base_radius = 10.0   # will be updated based on terminal size
        self.curr_radius = self.base_radius

        self.last_dims: Optional[tuple[int, int]] = None

        # Color seeds for the two orbs (changed each cycle)
        self.seed1 = random.randint(0, 10000)
        self.seed2 = random.randint(0, 10000)

        # Particles for explosions and reforming pieces
        self.explosion_particles: List[Dict] = []  # {x, y, vx, vy, life, seed}
        self.reform_particles: List[Dict] = []     # {x, y, vx, vy, life, seed}

    # ---------- helpers ----------
    def _reset_for_new_cycle(self, width: int, height: int):
        # radius scaled to terminal size
        self.base_radius = max(4.0, min(width, height) * 0.3)
        self.curr_radius = self.base_radius
        self.theta = random.uniform(0, 2 * math.pi)
        self.state_timer = 0.0
        self.phase = "orbit"
        self.explosion_particles.clear()
        self.reform_particles.clear()
        self.seed1 = random.randint(0, 10000)
        self.seed2 = random.randint(0, 10000)

    def _spawn_explosion(self, cx: float, cy: float, width: int, height: int):
        # big-ish radial burst at the center
        self.explosion_particles.clear()
        num = 80
        for _ in range(num):
            angle = random.uniform(0, 2 * math.pi)
            speed = random.uniform(10, 28) * self.speed_factor
            vx = speed * math.cos(angle)
            vy = speed * math.sin(angle)
            life = random.uniform(0.4, 0.9)
            # use both seeds so colors of fragments mix a bit
            seed = random.choice([self.seed1, self.seed2])
            self.explosion_particles.append({
                "x": cx,
                "y": cy,
                "vx": vx,
                "vy": vy,
                "life": life,
                "seed": seed,
            })

    def _spawn_reform_pieces(self, cx: float, cy: float, width: int, height: int):
        # pieces start further out and move back toward center
        self.reform_particles.clear()
        num = 70
        r_min = max(3.0, min(width, height) * 0.15)
        r_max = max(5.0, min(width, height) * 0.4)
        for _ in range(num):
            angle = random.uniform(0, 2 * math.pi)
            radius = random.uniform(r_min, r_max)
            x = cx + radius * math.cos(angle)
            y = cy + radius * math.sin(angle)
            # velocity towards center
            dir_x = cx - x
            dir_y = cy - y
            mag = math.hypot(dir_x, dir_y) or 1.0
            dir_x /= mag
            dir_y /= mag
            speed = random.uniform(8, 18) * self.speed_factor
            vx = dir_x * speed
            vy = dir_y * speed
            life = random.uniform(0.4, 0.9)
            seed = random.choice([self.seed1, self.seed2])
            self.reform_particles.append({
                "x": x,
                "y": y,
                "vx": vx,
                "vy": vy,
                "life": life,
                "seed": seed,
            })

    def _plot_ball(self, buf, x: float, y: float, radius: int, col: str):
        # Draw a small filled circle of characters around (x, y)
        cx = int(round(x))
        cy = int(round(y))
        height = len(buf)
        width = len(buf[0]) if height > 0 else 0
        for dy in range(-radius, radius + 1):
            for dx in range(-radius, radius + 1):
                if dx*dx + dy*dy <= radius*radius + 0.5:
                    px = cx + dx
                    py = cy + dy
                    if 0 <= px < width and 0 <= py < height:
                        ch = "O" if dx == 0 and dy == 0 else "o"
                        buf[py][px] = col + ch + RESET

    # ---------- main update/render ----------
    def update(self, dt: float, width: int, height: int, t_abs: float):
        dims = (width, height)
        if self.last_dims != dims:
            self.last_dims = dims
            self._reset_for_new_cycle(width, height)

        cx = width / 2.0
        cy = height / 2.0

        self.state_timer += dt

        if self.phase == "orbit":
            # orbit around center at fixed radius
            self.theta += 0.7 * self.speed_factor * dt
            if self.state_timer > 4.0:
                # start spiraling inward
                self.phase = "converge"
                self.state_timer = 0.0

        elif self.phase == "converge":
            # spiral inward until collision
            self.theta += 1.2 * self.speed_factor * dt
            shrink_rate = (self.base_radius / 1.2) * self.speed_factor
            self.curr_radius = max(0.0, self.curr_radius - shrink_rate * dt)
            if self.curr_radius <= 0.8:
                # collision at center
                self.phase = "explode"
                self.state_timer = 0.0
                self._spawn_explosion(cx, cy, width, height)

        elif self.phase == "explode":
            # update explosion particle positions
            new_parts = []
            for p in self.explosion_particles:
                p["x"] += p["vx"] * dt
                p["y"] += p["vy"] * dt
                p["life"] -= dt
                if p["life"] > 0:
                    new_parts.append(p)
            self.explosion_particles = new_parts

            if self.state_timer > 0.8 or not self.explosion_particles:
                # start reforming from outer pieces back to center
                self.phase = "reform"
                self.state_timer = 0.0
                self._spawn_reform_pieces(cx, cy, width, height)

        elif self.phase == "reform":
            new_parts = []
            for p in self.reform_particles:
                p["x"] += p["vx"] * dt
                p["y"] += p["vy"] * dt
                p["life"] -= dt
                if p["life"] > 0:
                    new_parts.append(p)
            self.reform_particles = new_parts

            if self.state_timer > 1.2 or not self.reform_particles:
                # new orbiting cycle with fresh colors
                self._reset_for_new_cycle(width, height)

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = [[" " for _ in range(width)] for _ in range(height)]
        cx = width / 2.0
        cy = height / 2.0

        # --- explosion particles ---
        for p in self.explosion_particles:
            x = int(p["x"])
            y = int(p["y"])
            if 0 <= x < width and 0 <= y < height:
                col = self.color_provider.get(t_abs, p["seed"])
                ch = random.choice("*+x0123456789")
                buf[y][x] = col + ch + RESET

        # --- reform particles ---
        for p in self.reform_particles:
            x = int(p["x"])
            y = int(p["y"])
            if 0 <= x < width and 0 <= y < height:
                col = self.color_provider.get(t_abs, p["seed"])
                ch = random.choice(".o*")
                buf[y][x] = col + ch + RESET

        # --- orbs (during orbit / converge, and fade-in at end of reform) ---
        show_orbs = self.phase in ("orbit", "converge")
        fade_in_center = self.phase == "reform"

        if show_orbs:
            r = self.curr_radius
            # positions on opposite sides of orbit
            x1 = cx + r * math.cos(self.theta)
            y1 = cy + r * math.sin(self.theta)
            x2 = cx - r * math.cos(self.theta)
            y2 = cy - r * math.sin(self.theta)

            col1 = self.color_provider.get(t_abs, self.seed1)
            col2 = self.color_provider.get(t_abs, self.seed2)
            self._plot_ball(buf, x1, y1, radius=2, col=col1)
            self._plot_ball(buf, x2, y2, radius=2, col=col2)

        elif fade_in_center:
            # towards end of reform, fade the merged orb in the center
            fade = min(1.0, max(0.0, self.state_timer / 1.0))
            if fade > 0.2:
                # mix both seeds for color variation
                seed = (self.seed1 + self.seed2) // 2
                col = self.color_provider.get(t_abs, seed)
                # DIM at the start, full towards the end
                if fade < 0.7:
                    col = DIM + col
                self._plot_ball(buf, cx, cy, radius=2, col=col)

        return "\n".join("".join(row) for row in buf)