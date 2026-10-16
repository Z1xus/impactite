import { css, threshold } from "#lib/dither/bayer.ts";

// the form breaks into pixels that arc into the meteor above it, then resolves
export function absorb(form: HTMLElement): Promise<void> {
	const rock = form.closest("main")?.querySelector("canvas");
	if (!rock || matchMedia("(prefers-reduced-motion: reduce)").matches) return Promise.resolve();

	const CELL = 3;
	const [STAGGER, TRAVEL, PULSE] = [160, 360, 160];
	const canvas = document.createElement("canvas");
	canvas.className = "pixelated pointer-events-none fixed inset-0 z-50 h-full w-full";
	const [w, h] = [Math.ceil(innerWidth / CELL), Math.ceil(innerHeight / CELL)];
	canvas.width = w;
	canvas.height = h;
	document.body.append(canvas);
	const g = canvas.getContext("2d");
	if (!g) {
		canvas.remove();
		return Promise.resolve();
	}
	const [fg, dim, edge, cyan, deep] = [
		"--color-fg",
		"--color-dim",
		"--color-edge",
		"--color-cyan",
		"--color-cyan-deep"
	].map(css);

	const box = rock.getBoundingClientRect();
	const [tx, ty] = [(box.left + box.width / 2) / CELL, (box.top + box.height / 2) / CELL];
	const radius = box.width / CELL / 2;

	type Pixel = { x: number; y: number; cx: number; cy: number; delay: number; color: string };
	const pixels: Pixel[] = [];
	const area = form.getBoundingClientRect();
	for (const part of form.querySelectorAll<HTMLElement>("input, button, a, span, p")) {
		const r = part.getBoundingClientRect();
		if (!r.width || !r.height) continue;
		const solid =
			part.tagName === "BUTTON" &&
			getComputedStyle(part).backgroundColor !== "rgba(0, 0, 0, 0)";
		const field = part.tagName === "INPUT";
		for (let y = r.top; y < r.bottom; y += CELL * 2)
			for (let x = r.left; x < r.right; x += CELL * 2) {
				const border =
					x - r.left < CELL * 2 ||
					r.right - x < CELL * 2 ||
					y - r.top < CELL * 2 ||
					r.bottom - y < CELL * 2;
				if (!solid && !(field && border) && Math.random() > 0.2) continue;
				const [px, py] = [x / CELL, y / CELL];
				// the curve bows sideways, alternating, so the stream spirals in
				const side = (px < tx ? -1 : 1) * (0.4 + Math.random() * 0.4);
				pixels.push({
					x: px,
					y: py,
					cx: (px + tx) / 2 + (ty - py) * side,
					cy: (py + ty) / 2,
					// the top of the form leaves first
					delay:
						((y - area.top) / Math.max(1, area.height)) * STAGGER * 0.7 +
						Math.random() * STAGGER * 0.3,
					color: solid ? cyan : field ? edge : Math.random() < 0.5 ? fg : dim
				});
			}
	}
	form.style.visibility = "hidden";

	const at = (p: Pixel, t: number) => {
		const u = 1 - t;
		return [
			u * u * p.x + 2 * u * t * p.cx + t * t * tx,
			u * u * p.y + 2 * u * t * p.cy + t * t * ty
		];
	};

	return new Promise((resolve) => {
		const start = performance.now();
		const step = (now: number) => {
			const time = now - start;
			g.clearRect(0, 0, w, h);
			for (const p of pixels) {
				const t = Math.min(1, Math.max(0, (time - p.delay) / TRAVEL));
				if (t >= 1) continue;
				const eased = t * t * (3 - 2 * t);
				// a short streak behind each pixel, brighter as it nears the rock
				for (let k = 0; k < 3; k++) {
					const [x, y] = at(p, Math.max(0, eased - k * 0.04)).map(Math.round);
					if (k && 1 - k / 3 <= threshold(x, y)) continue;
					g.fillStyle = eased > 0.75 ? cyan : eased > 0.4 && k ? deep : p.color;
					g.fillRect(x, y, 1, 1);
				}
			}
			const arrived = Math.min(1, Math.max(0, (time - STAGGER) / TRAVEL));
			rock.style.transform = `scale(${1 + arrived * arrived * 0.06})`;
			const pulse = (time - STAGGER - TRAVEL) / PULSE;
			if (pulse > 0 && pulse < 1) {
				const ring = radius * (0.9 + pulse * 0.6);
				for (let y = Math.floor(ty - ring - 3); y < ty + ring + 3; y++)
					for (let x = Math.floor(tx - ring - 3); x < tx + ring + 3; x++) {
						const off = Math.abs(Math.hypot(x - tx, y - ty) - ring);
						if (off > 2 || (1 - pulse) * (1 - off / 2) <= threshold(x, y)) continue;
						g.fillStyle = off < 1 ? cyan : deep;
						g.fillRect(x, y, 1, 1);
					}
			}
			if (pulse < 1) requestAnimationFrame(step);
			else {
				canvas.remove();
				rock.style.transform = "";
				resolve();
			}
		};
		requestAnimationFrame(step);
	});
}
