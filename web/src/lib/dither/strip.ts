import type { Cell } from "#lib/uptime/types.ts";
import { css } from "./bayer";

export const COLORS = [
	"--color-cyan",
	"--color-warn",
	"--color-bad",
	"--color-faint",
	"--color-control",
	"--color-fg"
];

const [CYAN, WARN, BAD, FAINT, CONTROL, FG] = COLORS.map((_, i) => i + 1);

export const QUIET = [FAINT, CONTROL];

// art pixels per cell, the gap between cells, and the rows of a cell and of the mark under it
export const WIDTH = 2;
export const GAP = 1;
export const ROWS = 9;
export const HEIGHT = ROWS + 2;

// a day with downtime always shows at least this many rows, the tooltip has the exact share
const FLOOR = 5;

function lost(cell: NonNullable<Cell>) {
	const share = 1 - cell[0];
	return cell[1] === "down" ? FLOOR + share * (ROWS - FLOOR) : share * ROWS;
}

export function fit(room: number, gap: number, minimum: number, days: number, stretch: boolean) {
	const wide = stretch ? Math.max(minimum, Math.floor((room + gap) / days) - gap) : minimum;
	const count = Math.max(1, Math.min(days, Math.floor((room + gap) / (wide + gap))));
	return { wide, count, total: count * (wide + gap) - gap };
}

export function strip(cells: Cell[], paused: boolean, hover: number | null, wide = WIDTH) {
	const columns = cells.length * (wide + GAP) - GAP;
	const data = new Uint8Array(Math.max(0, columns) * HEIGHT);
	cells.forEach((cell, i) => {
		const from = i * (wide + GAP);
		for (let x = from; x < from + wide; x++) {
			for (let y = 0; y < ROWS; y++) {
				const at = y * columns + x;
				if (!cell) data[at] = FAINT;
				else if (paused) data[at] = CONTROL;
				else {
					data[at] = y < Math.round(lost(cell)) ? BAD : cell[1] === "slow" ? WARN : CYAN;
				}
			}
			if (i === hover) data[(HEIGHT - 1) * columns + x] = FG;
		}
	});
	return data;
}

// device pixels, so every cell has the same width and gap
export function smooth(
	ctx: CanvasRenderingContext2D,
	cells: Cell[],
	paused: boolean,
	hover: number | null,
	[wide, gap, height, ratio]: number[]
) {
	const [cyan, warn, bad, faint, control, fg] = COLORS.map(css);
	const tall = height - 2 * ratio - 2 * ratio;
	const round = (x: number, y: number, w: number, h: number) => {
		ctx.beginPath();
		ctx.roundRect(x, y, w, h, 1.5 * ratio);
		ctx.fill();
	};
	cells.forEach((cell, i) => {
		const x = i * (wide + gap);
		if (!cell) {
			ctx.fillStyle = `${faint}55`;
			round(x, 0, wide, tall);
			return;
		}
		ctx.save();
		ctx.beginPath();
		ctx.roundRect(x, 0, wide, tall, 1.5 * ratio);
		ctx.clip();
		ctx.fillStyle = paused ? `${control}88` : cell[1] === "slow" ? warn : cyan;
		ctx.fillRect(x, 0, wide, tall);
		if (!paused) {
			ctx.fillStyle = bad;
			ctx.fillRect(x, 0, wide, Math.round((lost(cell) / ROWS) * tall));
		}
		ctx.restore();
		if (i === hover) {
			ctx.fillStyle = fg;
			round(x, height - 2 * ratio, wide, 2 * ratio);
		}
	});
}
