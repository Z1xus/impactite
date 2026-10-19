import { prefersReducedMotion } from "svelte/motion";
import { css, threshold } from "#lib/dither/bayer.ts";

// the meteor falls into the hero, and its shock ring wakes the preview below
export function impact(stage: HTMLElement, landed: (value: boolean) => void) {
	const rock = stage.querySelector<HTMLElement>("[data-rock]");
	if (!rock) return;
	const show = () => {
		stage.dataset.landed = "true";
		rock.style.opacity = "1";
		rock.style.transform = "";
	};
	if (prefersReducedMotion.current) return;
	landed(false);

	const CELL = 4;
	const canvas = document.createElement("canvas");
	canvas.className = "pixelated pointer-events-none fixed inset-0 z-10 h-full w-full";
	document.body.append(canvas);
	const target = rock.getBoundingClientRect();
	const [w, h] = [Math.ceil(innerWidth / CELL), Math.ceil(innerHeight / CELL)];
	canvas.width = w;
	canvas.height = h;
	const g = canvas.getContext("2d");
	if (!g) {
		canvas.remove();
		show();
		landed(true);
		return;
	}
	const [fg, cyan, deep] = ["--color-fg", "--color-cyan", "--color-cyan-deep"].map(css);
	const end = [(target.left + target.width / 2) / CELL, (target.top + target.height / 2) / CELL];
	const radius = target.width / CELL / 2;
	const start = [end[0] - 150, -radius];
	const along = (t: number) => [
		start[0] + (end[0] - start[0]) * t,
		start[1] + (end[1] - start[1]) * t
	];

	const [DELAY, FALL, RING] = [250, 650, 900];
	const debris = Array.from({ length: 48 }, () => {
		const angle = Math.random() * Math.PI * 2;
		const speed = radius * (1.2 + Math.random() * 2.4);
		return {
			vx: Math.cos(angle) * speed,
			vy: Math.sin(angle) * speed - radius,
			life: 0.6 + Math.random() * 0.4,
			hot: Math.random() < 0.3
		};
	});
	let begin = 0;
	let done = false;
	const draw = (now: number) => {
		begin ||= now;
		const time = now - begin - DELAY;
		g.clearRect(0, 0, w, h);
		if (time < FALL) {
			// accelerating, the way something falls
			const t = Math.max(0, time) / FALL;
			const head = along(t * t);
			rock.style.opacity = "1";
			rock.style.transform = `translate(${(head[0] - end[0]) * CELL}px, ${
				(head[1] - end[1]) * CELL
			}px) scale(${0.6 + 0.4 * t})`;
			// a tapered stream behind the rock, longer as it speeds up
			const size = radius * (0.6 + 0.4 * t);
			const [ux, uy] = [start[0] - end[0], start[1] - end[1]];
			const unit = Math.hypot(ux, uy);
			const [dx, dy] = [ux / unit, uy / unit];
			const length = 30 + 90 * t;
			for (let j = 0; j < length; j++) {
				const rest = 1 - j / length;
				const half = 1 + size * 0.45 * rest;
				for (let o = -half; o <= half; o++) {
					const x = Math.round(head[0] + dx * (size * 0.7 + j) - dy * o);
					const y = Math.round(head[1] + dy * (size * 0.7 + j) + dx * o);
					const heat = rest * (1 - Math.abs(o) / (half + 1));
					if (heat <= threshold(x, y)) continue;
					g.fillStyle = heat > 0.75 ? fg : heat > 0.4 ? cyan : deep;
					g.fillRect(x, y, 1, 1);
				}
			}
		} else {
			if (!done) {
				done = true;
				show();
				landed(true);
			}
			const p = (time - FALL) / RING;
			if (p >= 1) {
				canvas.remove();
				return;
			}
			// a dithered flash where it hit, a fast bright ring, a slower faint one, and debris
			const flash = Math.max(0, 1 - p * 6);
			const span = Math.hypot(w, h) * 0.9;
			const rings = [
				{
					at: radius * 0.9 + (1 - (1 - p) ** 3) * span,
					width: 3,
					strength: (1 - p) * 0.95
				},
				{
					at: radius * 0.9 + (1 - (1 - Math.max(0, p - 0.12)) ** 2) * span * 0.6,
					width: 2,
					strength: Math.max(0, 1 - p * 1.3) * 0.5
				}
			];
			for (let y = 0; y < h; y++)
				for (let x = 0; x < w; x++) {
					const d = Math.hypot(x - end[0], y - end[1]);
					const t = threshold(x, y);
					if (flash && d < radius * 1.4 && flash * (1 - d / (radius * 1.4)) > t) {
						g.fillStyle = d < radius * 0.7 ? fg : cyan;
						g.fillRect(x, y, 1, 1);
						continue;
					}
					for (const ring of rings) {
						const off = Math.abs(d - ring.at);
						if (off > ring.width || ring.strength * (1 - off / ring.width) <= t)
							continue;
						g.fillStyle = off < 1 ? cyan : deep;
						g.fillRect(x, y, 1, 1);
						break;
					}
				}
			for (const bit of debris) {
				const age = (time - FALL) / 1000;
				const [x, y] = [
					Math.round(end[0] + bit.vx * age),
					Math.round(end[1] + bit.vy * age + 60 * age * age)
				];
				if (bit.life * (1 - p) <= threshold(x, y)) continue;
				g.fillStyle = bit.hot ? fg : cyan;
				g.fillRect(x, y, 1, 1);
			}
		}
		requestAnimationFrame(draw);
	};
	requestAnimationFrame(draw);
}
