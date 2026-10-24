import { css, threshold } from "./bayer";

export const COLORS = [
	"--color-cyan",
	"--color-cyan-deep",
	"--color-cyan-night",
	"--color-line",
	"--color-faint",
	"--color-fg",
	"--color-bad"
];

const [LINE, DEEP, NIGHT, GRID, FAINT, FG, BAD] = COLORS.map((_, i) => i + 1);

export type Band = { from: number; to: number; label?: string; notes?: string[] };
export type Mark = { at: number; tone: "note" | "spike"; lines: string[] };

const edge = (index: number, count: number, columns: number) =>
	Math.round((index / Math.max(1, count - 1)) * (columns - 1));

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
	at = (x: number, width: number) => sample(values, x, width),
	bands: Band[] = [],
	marks: Mark[] = []
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

	for (const band of bands) {
		const start = edge(band.from, values.length, columns);
		const end = Math.max(start + 1, edge(band.to, values.length, columns));
		for (let x = start; x <= end; x++)
			for (let y = 0; y < rows; y++)
				if (x === start || x === end || threshold(x, y) < 0.3) put(x, y, BAD);
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

	for (const mark of marks) {
		const x = edge(mark.at, values.length, columns);
		if (mark.tone === "note") for (let y = 4; y < rows; y += 3) put(x, y, FAINT);
		for (let dx = -1; dx <= 1; dx++)
			for (let dy = 0; dy < 3; dy++) put(x + dx, dy, mark.tone === "note" ? FG : LINE);
	}

	if (hover !== null && values.length) {
		const x = Math.round((hover / Math.max(1, values.length - 1)) * (columns - 1));
		for (let y = 0; y < rows; y += 2) put(x, y, FAINT);
		const y = top(values[hover] ?? 0);
		for (let dx = -1; dx <= 1; dx++) for (let dy = -1; dy <= 1; dy++) put(x + dx, y + dy, FG);
	}

	return data;
}

export function smooth(
	ctx: CanvasRenderingContext2D,
	width: number,
	height: number,
	peak: number,
	axes: boolean,
	values: number[],
	previous: number[],
	hover: number | null,
	at: (x: number, width: number) => number,
	bands: Band[] = [],
	marks: Mark[] = []
) {
	const [cyan, grid, faint, fg, panel, bad] = [
		"--color-cyan",
		"--color-line",
		"--color-faint",
		"--color-fg",
		"--color-panel",
		"--color-bad"
	].map(css);
	const top = (value: number) => 1 + (1 - value / peak) * (height - 2);
	const trace = (series: (x: number) => number) => {
		ctx.beginPath();
		for (let x = 0; x < width; x++) ctx.lineTo(x, top(series(x)));
	};
	ctx.lineWidth = 1;
	ctx.lineJoin = "round";

	if (axes) {
		ctx.strokeStyle = grid;
		ctx.setLineDash([2, 3]);
		ctx.beginPath();
		for (const tick of [peak, peak / 2, 0]) {
			const y = Math.round(top(tick)) + 0.5;
			ctx.moveTo(0, y);
			ctx.lineTo(width, y);
		}
		ctx.stroke();
	}

	ctx.fillStyle = `${bad}30`;
	for (const band of bands) {
		const start = edge(band.from, values.length, width);
		ctx.fillRect(start, 0, Math.max(2, edge(band.to, values.length, width) - start), height);
	}

	trace((x) => at(x, width));
	ctx.lineTo(width - 1, height);
	ctx.lineTo(0, height);
	const fill = ctx.createLinearGradient(0, 0, 0, height);
	fill.addColorStop(0, `${cyan}38`);
	fill.addColorStop(1, `${cyan}00`);
	ctx.fillStyle = fill;
	ctx.fill();
	ctx.setLineDash([]);
	ctx.lineWidth = 1.5;
	ctx.strokeStyle = cyan;
	trace((x) => at(x, width));
	ctx.stroke();

	ctx.lineWidth = 1;
	ctx.strokeStyle = faint;
	ctx.setLineDash([3, 3]);
	if (previous.length) {
		trace((x) => sample(previous, x, width));
		ctx.stroke();
	}

	for (const mark of marks) {
		const x = Math.round(edge(mark.at, values.length, width)) + 0.5;
		if (mark.tone === "note") {
			ctx.beginPath();
			ctx.moveTo(x, 8);
			ctx.lineTo(x, height);
			ctx.stroke();
		}
		ctx.beginPath();
		ctx.arc(x, 4, 3, 0, Math.PI * 2);
		ctx.fillStyle = mark.tone === "note" ? fg : cyan;
		ctx.fill();
	}

	if (hover !== null && values.length) {
		const x = Math.round((hover / Math.max(1, values.length - 1)) * (width - 1)) + 0.5;
		ctx.beginPath();
		ctx.moveTo(x, 0);
		ctx.lineTo(x, height);
		ctx.stroke();
		ctx.beginPath();
		ctx.arc(x, top(values[hover] ?? 0), 3.5, 0, Math.PI * 2);
		ctx.lineWidth = 3;
		ctx.strokeStyle = panel;
		ctx.setLineDash([]);
		ctx.stroke();
		ctx.fillStyle = fg;
		ctx.fill();
	}
}
