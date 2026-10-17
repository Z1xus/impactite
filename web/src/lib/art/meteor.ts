const BAYER = [
	0, 32, 8, 40, 2, 34, 10, 42, 48, 16, 56, 24, 50, 18, 58, 26, 12, 44, 4, 36, 14, 46, 6, 38, 60,
	28, 52, 20, 62, 30, 54, 22, 3, 35, 11, 43, 1, 33, 9, 41, 51, 19, 59, 27, 49, 17, 57, 25, 15, 47,
	7, 39, 13, 45, 5, 37, 63, 31, 55, 23, 61, 29, 53, 21
];
export const PALETTE = [
	[11, 26, 30],
	[6, 52, 59],
	[14, 127, 141],
	[56, 225, 242],
	[227, 238, 240]
];
export const LIGHT = normalize([-0.55, -0.6, 0.58]);

type Vec = [number, number, number];

export function normalize([x, y, z]: number[]): Vec {
	const length = Math.hypot(x, y, z) || 1;
	return [x / length, y / length, z / length];
}

function dot(a: Vec, b: Vec) {
	return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

function hash(a: number, b: number) {
	const s = Math.sin(a * 127.1 + b * 311.7) * 43758.5453;
	return s - Math.floor(s);
}

const craters = Array.from({ length: 16 }, (_, i) => {
	const lon = hash(i, 1) * Math.PI * 2;
	const lat = Math.asin(hash(i, 2) * 2 - 1);
	const center: Vec = [
		Math.cos(lat) * Math.sin(lon),
		Math.sin(lat),
		Math.cos(lat) * Math.cos(lon)
	];
	return { center, radius: 0.12 + hash(i, 3) * 0.26 };
});

function radiusAt(angle: number) {
	return (
		0.8 +
		0.05 * Math.sin(angle * 3 + 0.7) +
		0.03 * Math.sin(angle * 5 + 2.1) +
		0.015 * Math.sin(angle * 11 + 4.2)
	);
}

export function meteor(n: number, turn: number, light: Vec = LIGHT) {
	const cos = Math.cos(turn);
	const sin = Math.sin(turn);
	const data = new Uint8Array(n * n);
	for (let y = 0; y < n; y++) {
		for (let x = 0; x < n; x++) {
			const i = y * n + x;
			const nx = ((x + 0.5) / n) * 2 - 1;
			const ny = ((y + 0.5) / n) * 2 - 1;
			const edge = radiusAt(Math.atan2(ny, nx));
			const r = Math.hypot(nx, ny) / edge;
			if (r >= 1) {
				data[i] = 0;
				continue;
			}
			let normal: Vec = [nx / edge, ny / edge, Math.sqrt(1 - r * r)];
			const body: Vec = [
				normal[0] * cos - normal[2] * sin,
				normal[1],
				normal[0] * sin + normal[2] * cos
			];

			let shade = 0;
			for (const crater of craters) {
				const distance = Math.acos(Math.min(1, dot(body, crater.center)));
				if (distance > crater.radius * 1.25) continue;
				const t = distance / crater.radius;
				const toward = normalize([
					crater.center[0] - body[0],
					crater.center[1] - body[1],
					crater.center[2] - body[2]
				]);
				const view: Vec = [
					toward[0] * cos + toward[2] * sin,
					toward[1],
					-toward[0] * sin + toward[2] * cos
				];
				const wall = t < 1 ? 0.9 * t : -0.5 * (1.25 - t) * 4;
				normal = normalize([
					normal[0] + view[0] * wall,
					normal[1] + view[1] * wall,
					normal[2] + view[2] * wall
				]);
				shade -= t < 1 ? 0.12 * (1 - t) : 0;
			}

			const grain =
				hash(
					Math.floor(body[0] * 14),
					Math.floor(body[1] * 14) + Math.floor(body[2] * 14) * 7
				) *
					0.12 -
				0.06;
			const lit = Math.max(0, dot(normal, light)) * 0.95 + 0.05 + shade + grain;
			const level = lit * (PALETTE.length - 1) + BAYER[(y % 8) * 8 + (x % 8)] / 64 - 0.5;
			data[i] = Math.max(0, Math.min(PALETTE.length - 1, Math.round(level))) + 1;
		}
	}
	return data;
}
