import { threshold } from "#lib/dither/bayer.ts";

export type Spec =
	| { kind: "brand"; path: string; hex: string }
	| { kind: "image"; url: string; ratio?: number }
	| { kind: "pixel"; svg: string }
	| { kind: "letter"; text: string };

const SIZE = 16;
const SUPER = 4;
const PALETTE = [
	"#1d2b33",
	"#1d2b53",
	"#7e2553",
	"#008751",
	"#ab5236",
	"#5f574f",
	"#c2c3c7",
	"#fff1e8",
	"#ff004d",
	"#ffa300",
	"#ffec27",
	"#00e436",
	"#29adff",
	"#83769c",
	"#ff77a8",
	"#ffccaa",
	"#38e1f2",
	"#e3eef0"
].map((hex) => {
	const n = parseInt(hex.slice(1), 16);
	return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
});

const cache = new Map<string, Promise<ImageData | null>>();

export function key(spec: Spec) {
	switch (spec.kind) {
		case "brand":
			return `b:${spec.hex}:${spec.path.slice(0, 64)}`;
		case "image":
			return `i:${spec.url}`;
		case "pixel":
			return `p:${spec.svg.length}:${spec.svg.slice(-64)}`;
		case "letter":
			return `l:${spec.text}`;
	}
}

export function render(spec: Spec): Promise<ImageData | null> {
	const id = key(spec);
	let pending = cache.get(id);
	if (!pending) {
		pending = draw(spec).catch(() => null);
		cache.set(id, pending);
	}
	return pending;
}

async function draw(spec: Spec): Promise<ImageData | null> {
	if (spec.kind === "pixel") return pixel(spec.svg);
	const big = document.createElement("canvas");
	big.width = big.height = SIZE * SUPER;
	const ctx = big.getContext("2d");
	if (!ctx) return null;
	const span = SIZE * SUPER;

	if (spec.kind === "brand") {
		ctx.scale(span / 24, span / 24);
		ctx.fillStyle = visible(spec.hex);
		ctx.fill(new Path2D(spec.path));
	} else if (spec.kind === "image") {
		const image = await load(spec.url);
		const ratio = spec.ratio ?? 1;
		const height = span / ratio;
		ctx.drawImage(image, 0, (span - height) / 2, span, height);
	} else {
		await document.fonts.ready;
		ctx.fillStyle = "#e3eef0";
		ctx.font = `${span * 0.75}px "Departure Mono"`;
		ctx.textAlign = "center";
		ctx.textBaseline = "middle";
		ctx.fillText(spec.text.slice(0, 1).toUpperCase(), span / 2, span / 2 + span * 0.04);
	}
	return quantize(ctx.getImageData(0, 0, span, span));
}

function quantize(source: ImageData) {
	const out = new ImageData(SIZE, SIZE);
	for (let y = 0; y < SIZE; y++) {
		for (let x = 0; x < SIZE; x++) {
			let r = 0;
			let g = 0;
			let b = 0;
			let a = 0;
			for (let dy = 0; dy < SUPER; dy++) {
				for (let dx = 0; dx < SUPER; dx++) {
					const i = ((y * SUPER + dy) * SIZE * SUPER + x * SUPER + dx) * 4;
					const alpha = source.data[i + 3] / 255;
					r += source.data[i] * alpha;
					g += source.data[i + 1] * alpha;
					b += source.data[i + 2] * alpha;
					a += alpha;
				}
			}
			const coverage = a / (SUPER * SUPER);
			const t = threshold(x, y);
			if (coverage < t * 0.9 + 0.05) continue;
			const spread = (t - 0.5) * 48;
			const color = nearest(r / a + spread, g / a + spread, b / a + spread);
			const o = (y * SIZE + x) * 4;
			out.data[o] = color[0];
			out.data[o + 1] = color[1];
			out.data[o + 2] = color[2];
			out.data[o + 3] = 255;
		}
	}
	return out;
}

function nearest(r: number, g: number, b: number) {
	let best = PALETTE[0];
	let distance = Infinity;
	for (const color of PALETTE) {
		const d =
			(color[0] - r) ** 2 * 0.3 + (color[1] - g) ** 2 * 0.59 + (color[2] - b) ** 2 * 0.11;
		if (d < distance) {
			distance = d;
			best = color;
		}
	}
	return best;
}

function pixel(svg: string) {
	const canvas = document.createElement("canvas");
	canvas.width = canvas.height = SIZE;
	const ctx = canvas.getContext("2d");
	if (!ctx) return null;
	ctx.translate(2, 2);
	ctx.scale(0.5, 0.5);
	ctx.fillStyle = "#c3d0d4";
	for (const match of svg.matchAll(/ d="([^"]+)"/g)) ctx.fill(new Path2D(match[1]));
	return ctx.getImageData(0, 0, SIZE, SIZE);
}

function visible(hex: string) {
	const n = parseInt(hex, 16);
	const luma = (((n >> 16) & 255) * 0.3 + ((n >> 8) & 255) * 0.59 + (n & 255) * 0.11) / 255;
	return luma < 0.22 ? "#e3eef0" : `#${hex}`;
}

function load(url: string) {
	return new Promise<HTMLImageElement>((resolve, reject) => {
		const image = new Image();
		image.onload = () => resolve(image);
		image.onerror = reject;
		image.src = url;
	});
}
