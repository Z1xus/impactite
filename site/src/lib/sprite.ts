export type Vec = [number, number, number];

// materials: 1 body, 2 lamp, 3 recess, 4 metal, a steady lamp has tag 0
export type Shape = {
	sdf: (x: number, y: number, z: number) => number;
	material: (x: number, y: number, z: number) => [number, number];
	tilt: number;
	reach: number;
	bound: number;
	pose: number;
	sway: number;
};

type Frame = { normal: Float32Array; material: Uint8Array; tag: Uint8Array };

const FRAMES = 96;
const TURN = Math.PI * 2;
const AROUND = [
	[1, 0],
	[-1, 0],
	[0, 1],
	[0, -1]
];

export function normalize([x, y, z]: number[]): Vec {
	const length = Math.hypot(x, y, z) || 1;
	return [x / length, y / length, z / length];
}

function bake(shape: Shape, n: number, angle: number): Frame {
	const [ct, st] = [Math.cos(shape.tilt), Math.sin(shape.tilt)];
	const [ca, sa] = [Math.cos(angle), Math.sin(angle)];
	// camera space is x right, y up, z toward the viewer
	const toObject = (x: number, y: number, z: number): Vec => {
		const [y1, z1] = [y * ct + z * st, -y * st + z * ct];
		return [x * ca - z1 * sa, y1, x * sa + z1 * ca];
	};
	const toCamera = (x: number, y: number, z: number): Vec => {
		const [x1, z1] = [x * ca + z * sa, -x * sa + z * ca];
		return [x1, y * ct - z1 * st, y * st + z1 * ct];
	};
	const frame: Frame = {
		normal: new Float32Array(n * n * 3),
		material: new Uint8Array(n * n),
		tag: new Uint8Array(n * n)
	};
	const ray = toObject(0, 0, -1);
	const e = 0.004;
	for (let py = 0; py < n; py++)
		for (let px = 0; px < n; px++) {
			const u = (((px + 0.5) / n) * 2 - 1) * shape.reach;
			const v = (1 - ((py + 0.5) / n) * 2) * shape.reach;
			// only march inside the bounding sphere
			const side = shape.bound ** 2 - u * u - v * v;
			if (side <= 0) continue;
			const origin = toObject(u, v, 3);
			const end = 3 + Math.sqrt(side);
			let t = 3 - Math.sqrt(side);
			let d = 1;
			for (let step = 0; step < 64 && t < end && d >= 0.002; step++) {
				d = shape.sdf(
					origin[0] + ray[0] * t,
					origin[1] + ray[1] * t,
					origin[2] + ray[2] * t
				);
				if (d >= 0.002) t += d;
			}
			if (d >= 0.002) continue;
			const [x, y, z] = [
				origin[0] + ray[0] * t,
				origin[1] + ray[1] * t,
				origin[2] + ray[2] * t
			];
			const i = py * n + px;
			frame.normal.set(
				toCamera(
					...normalize([
						shape.sdf(x + e, y, z) - shape.sdf(x - e, y, z),
						shape.sdf(x, y + e, z) - shape.sdf(x, y - e, z),
						shape.sdf(x, y, z + e) - shape.sdf(x, y, z - e)
					])
				),
				i * 3
			);
			[frame.material[i], frame.tag[i]] = shape.material(x, y, z);
		}
	return frame;
}

// level lights the tagged lamps in order and speeds up their blink
export function sprite(shape: Shape, n: number) {
	const frames = new Map<number, Frame>();
	return (angle: number, light: Vec, time: number, level: number) => {
		const index = Math.round(((((angle % TURN) + TURN) % TURN) / TURN) * FRAMES) % FRAMES;
		let frame = frames.get(index);
		if (!frame) {
			frame = bake(shape, n, (index / FRAMES) * TURN);
			frames.set(index, frame);
		}
		const { normal, material, tag } = frame;
		const out = new Uint8Array(n * n);
		const half = normalize([light[0], light[1], light[2] + 1]);
		for (let py = 0; py < n; py++)
			for (let px = 0; px < n; px++) {
				const i = py * n + px;
				const kind = material[i];
				if (!kind) continue;
				if (kind === 3) {
					out[i] = 1;
					continue;
				}
				if (kind === 2) {
					const order = (tag[i] * 0.618) % 1;
					const on = tag[i]
						? order < level && (time * (0.0004 + level * 0.0014) + order * 3) % 1 < 0.7
						: level > 0;
					out[i] = on ? 5 : 2;
					continue;
				}
				const [nx, ny, nz] = [normal[i * 3], normal[i * 3 + 1], normal[i * 3 + 2]];
				const diffuse = Math.max(0, nx * light[0] + ny * light[1] + nz * light[2]);
				const spec = Math.max(0, nx * half[0] + ny * half[1] + nz * half[2]) ** 28;
				const metal = kind === 4;
				const lit =
					(diffuse * 0.8 + 0.12) * (metal ? 1.12 : 1) + spec * (metal ? 0.7 : 0.3);
				const rim =
					nx * light[0] + ny * light[1] > 0.1 &&
					AROUND.some(
						([dx, dy]) =>
							dx * light[0] - dy * light[1] > 0.3 &&
							!material[(py + dy) * n + px + dx]
					);
				out[i] = rim ? 5 : lit > 0.72 ? 4 : lit > 0.38 ? 3 : 2;
			}
		return out;
	};
}
