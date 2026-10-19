import { threshold } from "./bayer.ts";
import { random } from "./crater.ts";

const TURN = Math.PI * 2;
const TONES = [0.85, 1.25, 1.65];
const pickSize = (n: number) => (n < 0.4 ? 0 : n < 0.85 ? 1 : 2);
const SIZES = [
	[1.8, 2.6],
	[3, 4.2],
	[5.2, 6.4]
];

export function rock(
	g: CanvasRenderingContext2D,
	cells: number,
	name: string,
	colors: Record<"floor" | "low" | "high" | "rim", string>
) {
	let seed = 2166136261;
	for (const char of name) seed = Math.imul(seed ^ char.codePointAt(0)!, 16777619);
	const next = random(seed >>> 0);
	const light = next() * TURN;
	const [lx, ly] = [Math.cos(light), Math.sin(light)];
	const tone = TONES[Math.floor(next() * 3)] + (next() - 0.5) * 0.1;
	const [ox, oy] = [Math.floor(next() * 8), Math.floor(next() * 8)];
	const stretch = 0.84 + next() * 0.32;
	const radius = cells * (0.38 + next() * 0.04);
	const bumps = [2, 3, 5].map((k) => ({ k, amp: 0.02 + next() * 0.05, phase: next() * TURN }));
	const edge = (angle: number) => {
		const lump = bumps.reduce((sum, b) => sum + b.amp * Math.cos(b.k * angle + b.phase), 0);
		const wide = radius * stretch * Math.cos(angle);
		const tall = (radius / stretch) * Math.sin(angle);
		return Math.min(cells / 2 - 0.5, (radius * radius) / Math.hypot(wide, tall)) * (1 + lump);
	};

	const craters: [number, number, number][] = [];
	const count = 1 + Number(next() > 0.4) + Number(next() > 0.85);
	for (let i = 0; i < count; i++) {
		const [min, max] = SIZES[pickSize(next())];
		const r = i && max > 5 ? 3.2 : min + next() * (max - min);
		for (let n = 0; n < 24; n++) {
			const angle = next() * TURN;
			const reach = next() * Math.max(0, edge(angle) - r - 1);
			const [cx, cy] = [
				cells / 2 + Math.cos(angle) * reach,
				cells / 2 + Math.sin(angle) * reach
			];
			if (craters.every(([x, y, other]) => Math.hypot(cx - x, cy - y) > r + other + 0.8)) {
				craters.push([cx, cy, r]);
				break;
			}
		}
	}

	for (let y = 0; y < cells; y++)
		for (let x = 0; x < cells; x++) {
			const px = x + 0.5 - cells / 2;
			const py = y + 0.5 - cells / 2;
			const dist = Math.hypot(px, py);
			const bound = edge(Math.atan2(py, px));
			if (dist > bound) continue;
			const t = threshold(x + ox, y + oy);
			const lit = (px * lx + py * ly) / radius;
			const value = tone + 0.5 * lit;
			let color = value - 1 > t ? colors.high : value > t ? colors.low : colors.floor;
			if (dist > bound - 1) color = lit > -0.3 ? colors.rim : colors.high;
			for (const [cx, cy, r] of craters) {
				const dx = x + 0.5 - cx;
				const dy = y + 0.5 - cy;
				const d = Math.hypot(dx, dy);
				const facing = (dx * lx + dy * ly) / Math.max(d, 1);
				if (d < r - 0.5)
					color = 0.5 + (0.5 * facing * d) / r > t ? colors.low : colors.floor;
				else if (d < r + 0.7) color = facing > -0.35 ? colors.rim : colors.high;
			}
			g.fillStyle = color;
			g.fillRect(x, y, 1, 1);
		}
}
