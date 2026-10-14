import type { Spec } from "./resolve.ts";

export type Mode = "true" | "cyan" | "mono";

const SUPER = 8;
const RAMPS: Record<Exclude<Mode, "true">, number[][]> = {
	cyan: [
		[14, 127, 141],
		[56, 225, 242],
		[227, 238, 240]
	],
	mono: [
		[76, 92, 100],
		[135, 152, 160],
		[227, 238, 240]
	]
};

const cache = new Map<string, Promise<ImageData | null>>();

export function pixelize(spec: Spec, cells: number, mode: Mode) {
	const id = `${cells}:${mode}:${spec.kind}:${"path" in spec ? spec.path.slice(0, 48) : "url" in spec ? spec.url : spec.svg.length}`;
	let pending = cache.get(id);
	if (!pending) {
		pending = draw(spec, cells, mode).catch(() => null);
		cache.set(id, pending);
	}
	return pending;
}

async function draw(spec: Spec, cells: number, mode: Mode) {
	const span = cells * SUPER;
	const canvas = document.createElement("canvas");
	canvas.width = canvas.height = span;
	const ctx = canvas.getContext("2d", { willReadFrequently: true });
	if (!ctx) return null;

	if (spec.kind === "brand") {
		ctx.scale(span / 24, span / 24);
		ctx.fillStyle = `#${spec.hex}`;
		ctx.fill(new Path2D(spec.path));
	} else if (spec.kind === "pixel") {
		ctx.scale(span / 24, span / 24);
		ctx.fillStyle = "#c3d0d4";
		for (const match of spec.svg.matchAll(/ d="([^"]+)"/g)) ctx.fill(new Path2D(match[1]));
	} else {
		const image = await load(spec.url);
		const height = spec.flag ? span * 0.75 : span;
		ctx.drawImage(image, 0, (span - height) / 2, span, height);
	}

	const source = ctx.getImageData(0, 0, span, span).data;
	const out = new ImageData(cells, cells);
	for (let y = 0; y < cells; y++) {
		for (let x = 0; x < cells; x++) {
			const votes = new Map<number, { weight: number; sum: number[] }>();
			let coverage = 0;
			for (let dy = 0; dy < SUPER; dy++) {
				for (let dx = 0; dx < SUPER; dx++) {
					const i = ((y * SUPER + dy) * span + x * SUPER + dx) * 4;
					const alpha = source[i + 3] / 255;
					if (alpha < 0.5) continue;
					coverage += alpha;
					const bucket =
						((source[i] >> 4) << 8) |
						((source[i + 1] >> 4) << 4) |
						(source[i + 2] >> 4);
					const vote = votes.get(bucket) ?? { weight: 0, sum: [0, 0, 0] };
					vote.weight += 1;
					vote.sum[0] += source[i];
					vote.sum[1] += source[i + 1];
					vote.sum[2] += source[i + 2];
					votes.set(bucket, vote);
				}
			}
			if (coverage / (SUPER * SUPER) < 0.45) continue;
			let winner = { weight: 0, sum: [0, 0, 0] };
			for (const vote of votes.values()) if (vote.weight > winner.weight) winner = vote;
			let color = winner.sum.map((channel) => channel / winner.weight);
			if (mode !== "true") {
				const ramp = RAMPS[mode];
				color =
					ramp[
						Math.min(
							ramp.length - 1,
							Math.floor(luma(color[0], color[1], color[2]) * ramp.length)
						)
					];
			} else if (spec.kind === "brand") {
				color = lift(color);
			}
			out.data.set([...color, 255], (y * cells + x) * 4);
		}
	}
	return out;
}

function luma(r: number, g: number, b: number) {
	return (r * 0.3 + g * 0.59 + b * 0.11) / 255;
}

function lift([r, g, b]: number[]) {
	return luma(r, g, b) < 0.2 ? [227, 238, 240] : [r, g, b];
}

function load(url: string) {
	return new Promise<HTMLImageElement>((resolve, reject) => {
		const image = new Image();
		image.onload = () => resolve(image);
		image.onerror = reject;
		image.src = url;
	});
}
