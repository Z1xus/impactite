<script lang="ts">
	import { css, threshold } from "#lib/dither/bayer.ts";
	import { crater, random } from "#lib/dither/crater.ts";
	import { theme } from "#lib/theme.svelte.ts";

	let { seed, size = 20 }: { seed: number; size?: number } = $props();

	const CELLS = 24;
	let canvas = $state<HTMLCanvasElement>();

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
		g.fillStyle = raised;
		g.fillRect(0, 0, CELLS, CELLS);
		g.fillStyle = night;
		for (let y = 0; y < CELLS; y++)
			for (let x = 0; x < CELLS; x++)
				if (
					0.15 + 0.1 * (1 + Math.cos(Math.atan2(y - 12, x - 12) - light)) >
					threshold(x, y)
				)
					g.fillRect(x, y, 1, 1);
		const spot: [number, number, number] = [
			9 + Math.floor(next() * 6),
			9 + Math.floor(next() * 6),
			4.5 + next() * 3
		];
		const rays = Array.from({ length: 7 }, () => next() * Math.PI * 2);
		crater(g, [CELLS, CELLS], spot, 1, rays, { night, deep, cyan });
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
