import type { Shape } from "./sprite.ts";

function box(x: number, y: number, z: number, bx: number, by: number, bz: number, r = 0) {
	const [qx, qy, qz] = [Math.abs(x) - bx + r, Math.abs(y) - by + r, Math.abs(z) - bz + r];
	return (
		Math.hypot(Math.max(qx, 0), Math.max(qy, 0), Math.max(qz, 0)) +
		Math.min(Math.max(qx, qy, qz), 0) -
		r
	);
}

const UNIT = 0.3;
const unit = (y: number) => Math.max(-2, Math.min(2, Math.round(y / UNIT)));

export const RACK: Shape = {
	tilt: 0.38,
	reach: 1.02,
	bound: 1.06,
	pose: -0.55,
	sway: 0.35,
	sdf: (x, y, z) => box(x, y, z, 0.48, 0.8, 0.4, 0.04),
	material(x, y, z) {
		if (z < 0.385) return [1, 0];
		const ry = y - unit(y) * UNIT;
		if (Math.abs(ry) > 0.13 || Math.abs(x) > 0.42) return [4, 0];
		if (Math.abs(x - 0.3) < 0.055 && Math.abs(ry) < 0.05) return [2, unit(y) + 3];
		if (x < 0.2 && x > -0.36 && Math.abs(ry) < 0.035) return [3, 0];
		return [1, 0];
	}
};

export const CRATE: Shape = {
	tilt: 0.5,
	reach: 1.06,
	bound: 1.02,
	pose: -0.62,
	sway: 0.3,
	sdf: (x, y, z) => box(x, y, z, 0.58, 0.58, 0.58, 0.03),
	material(x, y, z) {
		const [ax, ay] = [Math.abs(x), Math.abs(y)];
		const [a, b] = ax > 0.55 ? [z, y] : ay > 0.55 ? [x, z] : [x, y];
		if (Math.max(Math.abs(a), Math.abs(b)) > 0.46) return [4, 0];
		if (z > 0.55 && Math.abs(a) < 0.22 && Math.abs(b) < 0.1) return [2, 0];
		if (ay < 0.55 && Math.abs(((b + 1.2) % 0.31) - 0.155) > 0.135) return [3, 0];
		return [1, 0];
	}
};

export const CRATER: Shape = {
	tilt: 0.52,
	reach: 1.22,
	bound: 1.22,
	pose: Math.PI / 4,
	sway: 0.3,
	sdf(x, y, z) {
		const r = Math.hypot(x, z);
		const ground = box(x, y + 0.16, z, 0.8, 0.16, 0.8, 0.02);
		const rim = Math.hypot(r - 0.48, (y - 0.02) * 1.3) - 0.14;
		const bowl = Math.hypot(x, y - 0.42, z) - 0.52;
		const rock = Math.hypot(x, y + 0.02, z) - 0.17;
		return Math.min(Math.max(Math.min(ground, rim), -bowl), rock);
	},
	material(x, y, z) {
		const r = Math.hypot(x, z);
		if (r < 0.19 && y > -0.2) return [4, 0];
		if (r < 0.28 && y < -0.04) return [2, Math.floor(Math.atan2(z, x) * 2) + 8];
		if (Math.max(Math.abs(x), Math.abs(z)) > 0.78 && y < -0.02)
			return [Math.floor((y + 0.4) / 0.08) % 2 ? 1 : 3, 0];
		const ray = Math.abs(Math.sin(Math.atan2(z, x) * 4 + 0.4)) < 0.09;
		if (r > 0.64 && ray) return [3, 0];
		return [1, 0];
	}
};
