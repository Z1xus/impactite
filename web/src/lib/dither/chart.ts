import { threshold } from "./bayer";

export const COLORS = [
	"--color-cyan",
	"--color-cyan-deep",
	"--color-cyan-night",
	"--color-line",
	"--color-faint",
	"--color-fg"
];

const [LINE, DEEP, NIGHT, GRID, FAINT, FG] = COLORS.map((_, i) => i + 1);

export function sample(series: number[], x: number, columns: number) {
	if (series.length < 2) return series[0] ?? 0;
	const at = (x / (columns - 1)) * (series.length - 1);
	const low = Math.floor(at);
	const high = Math.min(series.length - 1, low + 1);
	return series[low] + (series[high] - series[low]) * (at - low);
}

export function chart(
	columns: number,
	rows: number,
	peak: number,
	axes: boolean,
	values: number[],
	previous: number[],
	hover: number | null = null,
	at = (x: number, width: number) => sample(values, x, width)
) {
	const ticks = [peak, peak / 2, 0];
	const data = new Uint8Array(columns * rows);
	const put = (x: number, y: number, color: number) => {
		if (x >= 0 && y >= 0 && x < columns && y < rows) data[y * columns + x] = color;
	};
	const top = (value: number) => Math.round((1 - value / peak) * (rows - 1));

	if (axes) {
		for (const tick of ticks) {
			const y = top(tick);
			for (let x = 0; x < columns; x += 2) put(x, y, GRID);
		}
	}

	let lastY = top(at(0, columns));
	for (let x = 0; x < columns; x++) {
		const y = top(at(x, columns));
		for (let yy = y + 1; yy < rows; yy++) {
			const depth = (yy - y) / Math.max(1, rows - y);
			const density = 0.3 * (1 - depth) ** 1.6;
			if (threshold(x, yy) < density) put(x, yy, depth < 0.18 ? DEEP : NIGHT);
		}
		for (let yy = Math.min(y, lastY); yy <= Math.max(y, lastY); yy++) put(x, yy, LINE);
		lastY = y;
	}

	if (previous.length) {
		for (let x = 0; x < columns; x += 2) put(x, top(sample(previous, x, columns)), FAINT);
	}

	if (hover !== null && values.length) {
		const x = Math.round((hover / Math.max(1, values.length - 1)) * (columns - 1));
		for (let y = 0; y < rows; y += 2) put(x, y, FAINT);
		const y = top(values[hover] ?? 0);
		for (let dx = -1; dx <= 1; dx++) for (let dy = -1; dy <= 1; dy++) put(x + dx, y + dy, FG);
	}

	return data;
}
