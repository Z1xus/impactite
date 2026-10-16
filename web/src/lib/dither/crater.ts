import { threshold } from "./bayer.ts";

export type Colors = { night: string; deep: string; cyan: string };

export function random(seed: number) {
	let state = seed * 2654435761;
	return () => {
		state = Math.imul(state ^ (state >>> 15), state | 1);
		state ^= state + Math.imul(state ^ (state >>> 7), state | 61);
		return ((state ^ (state >>> 14)) >>> 0) / 4294967296;
	};
}

// an impact seen from above, lit from the top left, `strength` fades it in from 0 to 1
export function crater(
	g: CanvasRenderingContext2D,
	[width, height]: [number, number],
	[cx, cy, radius]: [number, number, number],
	strength: number,
	rays: number[],
	colors: Colors
) {
	const turn = Math.PI * 2;
	for (let y = 0; y < height; y++)
		for (let x = 0; x < width; x++) {
			const dx = x - cx;
			const dy = (y - cy) * 1.15;
			const d = Math.hypot(dx, dy) / radius;
			if (d >= 2.2) continue;
			const angle = Math.atan2(dy, dx);
			const t = threshold(x, y);
			let color = "";
			if (d < 0.82) {
				const shade = 0.25 + 0.55 * Math.max(0, (-dx - dy) / (radius * 1.2));
				color =
					shade * strength > t ? colors.night : 0.18 * strength > t ? colors.deep : "";
			} else if (d < 1.08) {
				color =
					(0.5 + 0.5 * Math.cos(angle + 2.4)) * strength > t ? colors.cyan : colors.deep;
			} else if (
				rays.some(
					(r) => Math.abs(((((angle - r) % turn) + turn * 1.5) % turn) - Math.PI) < 0.08
				) &&
				(1 - (d - 1.08) / 1.12) * 0.7 * strength > t
			) {
				color = colors.deep;
			}
			if (!color) continue;
			g.fillStyle = color;
			g.fillRect(x, y, 1, 1);
		}
}
