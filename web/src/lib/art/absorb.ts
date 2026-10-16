import { css } from "#lib/dither/bayer.ts";

// the form breaks into pixels that swirl into the meteor above it, then resolves
export function absorb(form: HTMLElement): Promise<void> {
	const rock = form.closest("main")?.querySelector("canvas");
	if (!rock || matchMedia("(prefers-reduced-motion: reduce)").matches) return Promise.resolve();

	const CELL = 3;
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
	const reach = box.width / CELL / 3;

	type Particle = { x: number; y: number; vx: number; vy: number; color: string; gone: boolean };
	const particles: Particle[] = [];
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
				if (!solid && !(field && border) && Math.random() > 0.18) continue;
				particles.push({
					x: x / CELL,
					y: y / CELL,
					vx: (Math.random() - 0.5) * 1.6,
					vy: (Math.random() - 0.9) * 1.6,
					color: solid ? cyan : field ? edge : Math.random() < 0.5 ? fg : dim,
					gone: false
				});
			}
	}
	form.style.visibility = "hidden";

	return new Promise((resolve) => {
		const start = performance.now();
		const step = (now: number) => {
			const time = now - start;
			const pull = Math.min(1, time / 700) * 0.09;
			g.clearRect(0, 0, w, h);
			let left = 0;
			for (const p of particles) {
				if (p.gone) continue;
				const [dx, dy] = [tx - p.x, ty - p.y];
				const distance = Math.hypot(dx, dy) || 1;
				if (distance < reach) {
					p.gone = true;
					continue;
				}
				left++;
				// pulled in with a sideways swirl, so they spiral rather than fly straight
				p.vx = (p.vx + (dx / distance) * pull * 6 - (dy / distance) * 0.25) * 0.9;
				p.vy = (p.vy + (dy / distance) * pull * 6 + (dx / distance) * 0.25) * 0.9;
				p.x += p.vx;
				p.y += p.vy;
				g.fillStyle = distance < reach * 3 ? cyan : distance < reach * 6 ? deep : p.color;
				g.fillRect(Math.round(p.x), Math.round(p.y), 1, 1);
			}
			const absorbed = 1 - left / Math.max(1, particles.length);
			rock.style.transform = `scale(${1 + absorbed * 0.08})`;
			if (left && time < 1600) requestAnimationFrame(step);
			else {
				canvas.remove();
				rock.style.transform = "";
				resolve();
			}
		};
		requestAnimationFrame(step);
	});
}
