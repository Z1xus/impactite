import { css, threshold } from "#lib/dither/bayer.ts";

// the meteor falls into the hero, and its shock ring wakes the preview below
export function impact(stage: HTMLElement, landed: () => void) {
	const rock = stage.querySelector<HTMLElement>("[data-rock]");
	if (!rock) return;
	const show = () => {
		rock.style.opacity = "1";
		rock.style.transform = "";
	};
	if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
		show();
		landed();
		return;
	}

	const CELL = 4;
	const canvas = document.createElement("canvas");
	canvas.className = "pixelated pointer-events-none absolute inset-0 z-10 h-full w-full";
	stage.append(canvas);
	const box = stage.getBoundingClientRect();
	const target = rock.getBoundingClientRect();
	const [w, h] = [Math.ceil(box.width / CELL), Math.ceil(box.height / CELL)];
	canvas.width = w;
	canvas.height = h;
	const g = canvas.getContext("2d");
	if (!g) {
		canvas.remove();
		show();
		landed();
		return;
	}
	const [fg, cyan, deep] = ["--color-fg", "--color-cyan", "--color-cyan-deep"].map(css);
	const end = [
		(target.left - box.left + target.width / 2) / CELL,
		(target.top - box.top + target.height / 2) / CELL
	];
	const radius = target.width / CELL / 2;
	// down from the upper middle, so the trail crosses the open space beside the headline
	const start = [end[0] - 150, -radius];
	const along = (t: number) => [
		start[0] + (end[0] - start[0]) * t,
		start[1] + (end[1] - start[1]) * t
	];

	const [DELAY, FALL, RING] = [250, 650, 750];
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
			rock.style.transform = `translate(${Math.round(((head[0] - end[0]) * CELL) / 4) * 4}px, ${
				Math.round(((head[1] - end[1]) * CELL) / 4) * 4
			}px) scale(${0.35 + 0.65 * t})`;
			// a tapered stream behind the rock, longer as it speeds up
			const size = radius * (0.35 + 0.65 * t);
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
				landed();
			}
			const p = (time - FALL) / RING;
			if (p >= 1) {
				canvas.remove();
				return;
			}
			const ring = radius * 0.9 + (1 - (1 - p) ** 3) * Math.hypot(w, h) * 0.9;
			const strength = (1 - p) * 0.9;
			for (let y = 0; y < h; y++)
				for (let x = 0; x < w; x++) {
					const off = Math.abs(Math.hypot(x - end[0], y - end[1]) - ring);
					if (off > 3 || strength * (1 - off / 3) <= threshold(x, y)) continue;
					g.fillStyle = off < 1 ? cyan : deep;
					g.fillRect(x, y, 1, 1);
				}
		}
		requestAnimationFrame(draw);
	};
	requestAnimationFrame(draw);
}
