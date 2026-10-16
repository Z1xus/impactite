<script lang="ts">
	import { css, threshold } from "#lib/dither/bayer.ts";
	import { theme } from "#lib/theme.svelte.ts";

	let { seed, size = 20 }: { seed: number; size?: number } = $props();

	const CELLS = 24;
	let canvas = $state<HTMLCanvasElement>();

	function random(seed: number) {
		let state = seed * 2654435761;
		return () => {
			state = Math.imul(state ^ (state >>> 15), state | 1);
			state ^= state + Math.imul(state ^ (state >>> 7), state | 61);
			return ((state ^ (state >>> 14)) >>> 0) / 4294967296;
		};
	}

	$effect(() => {
		void theme.shown;
		const g = canvas?.getContext("2d");
		if (!g) return;
		const [raised, night, deep, cyan] = [
			"--color-raised",
			"--color-cyan-night",
			"--color-cyan-deep",
			"--color-cyan"
		].map(css);
		const next = random(seed);
		const light = next() * Math.PI * 2;
		const cx = 9 + Math.floor(next() * 6);
		const cy = 9 + Math.floor(next() * 6);
		const radius = 4.5 + next() * 3;
		const rays = Array.from({ length: 7 }, () => next() * Math.PI * 2);
		g.fillStyle = raised;
		g.fillRect(0, 0, CELLS, CELLS);
		for (let y = 0; y < CELLS; y++)
			for (let x = 0; x < CELLS; x++) {
				const dx = x - cx;
				const dy = (y - cy) * 1.15;
				const d = Math.hypot(dx, dy) / radius;
				const angle = Math.atan2(dy, dx);
				const t = threshold(x, y);
				let color = "";
				if (d < 0.82) {
					const shade = 0.25 + 0.55 * Math.max(0, (-dx - dy) / (radius * 1.2));
					color = shade > t ? night : 0.18 > t ? deep : "";
				} else if (d < 1.08) {
					color = 0.5 + 0.5 * Math.cos(angle + 2.4) > t ? cyan : deep;
				} else if (d < 2.2) {
					const ray = rays.some(
						(r) =>
							Math.abs(
								((((angle - r) % (Math.PI * 2)) + Math.PI * 3) % (Math.PI * 2)) -
									Math.PI
							) < 0.08
					);
					if (ray && (1 - (d - 1.08) / 1.12) * 0.7 > t) color = deep;
				} else if (
					0.15 + 0.2 * (0.5 + 0.5 * Math.cos(Math.atan2(y - 12, x - 12) - light)) >
					t
				) {
					color = night;
				}
				if (!color) continue;
				g.fillStyle = color;
				g.fillRect(x, y, 1, 1);
			}
	});
</script>

<canvas
	bind:this={canvas}
	width={CELLS}
	height={CELLS}
	class="shrink-0 rounded-full pixelated"
	style:width="{size}px"
	style:height="{size}px"
	aria-hidden="true"
></canvas>
