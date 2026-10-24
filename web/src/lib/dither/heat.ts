import { css, threshold } from "./bayer";

const CELL = 4;
const RADIUS = 7;
const SPREAD = 2.6;
const COLORS = ["--color-cyan-deep", "--color-cyan", "--color-warn", "--color-bad"];

// clicks spread over nearby cells, so a busy button glows instead of showing single dots
export function density(clicks: number[][], width: number, height: number) {
	const columns = Math.ceil(width / CELL);
	const rows = Math.ceil(height / CELL);
	const field = new Float32Array(columns * rows);
	for (const [x, y, count] of clicks) {
		const cx = Math.round((width / 2 + x) / CELL);
		const cy = Math.round(y / CELL);
		for (let dy = -RADIUS; dy <= RADIUS; dy++)
			for (let dx = -RADIUS; dx <= RADIUS; dx++) {
				const px = cx + dx;
				const py = cy + dy;
				if (px < 0 || py < 0 || px >= columns || py >= rows) continue;
				field[py * columns + px] +=
					count * Math.exp(-(dx * dx + dy * dy) / (2 * SPREAD ** 2));
			}
	}
	const peak = field.reduce((max, value) => Math.max(max, value), 0) || 1;
	return { field: field.map((value) => value / peak), columns, rows };
}

const rgb = (name: string) => {
	const n = parseInt(css(name).slice(1), 16);
	return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
};

export function paint(
	ctx: CanvasRenderingContext2D,
	{ field, columns, rows }: ReturnType<typeof density>,
	plain: boolean
) {
	const palette = COLORS.map(rgb);
	const image = ctx.createImageData(columns, rows);
	for (let y = 0; y < rows; y++)
		for (let x = 0; x < columns; x++) {
			const value = field[y * columns + x];
			if (value < 0.03) continue;
			const t = plain ? 0.5 : threshold(x, y);
			// dithered heat thins out where few clicks land
			if (!plain && value < t * 0.4) continue;
			const level = Math.min(3, Math.max(0, Math.floor(value * 4 + (t - 0.5) * 0.9)));
			const at = (y * columns + x) * 4;
			image.data.set(palette[level], at);
			image.data[at + 3] = plain ? Math.round(60 + value * 170) : 230;
		}
	ctx.putImageData(image, 0, 0);
}
