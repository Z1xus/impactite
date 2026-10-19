<script lang="ts">
	import { crater, random } from "#lib/dither/crater.ts";
	import { css, threshold } from "#lib/dither/bayer.ts";
	import { theme } from "#lib/theme.svelte.ts";

	let {
		name,
		invited = false,
		size = 20
	}: { name: string; invited?: boolean; size?: number } = $props();

	const CELLS = 24;
	let canvas = $state<HTMLCanvasElement>();

	$effect(() => {
		void theme.shown;
		const g = canvas?.getContext("2d");
		if (!g) return;
		const [night, deep, cyan] = ["--color-cyan-night", "--color-cyan-deep", "--color-cyan"].map(
			css
		);
		const next = random([...name].reduce((hash, char) => hash * 31 + char.charCodeAt(0), 7));
		const light = next() * Math.PI * 2;
		const spot: [number, number, number] = [
			9 + Math.floor(next() * 6),
			9 + Math.floor(next() * 6),
			6.5 + next() * 2
		];
		const rays = Array.from({ length: 7 }, () => next() * Math.PI * 2);
		g.clearRect(0, 0, CELLS, CELLS);
		g.save();
		g.beginPath();
		g.arc(CELLS / 2, CELLS / 2, CELLS / 2, 0, Math.PI * 2);
		g.clip();
		g.fillStyle = night;
		g.fillRect(0, 0, CELLS, CELLS);
		g.fillStyle = deep;
		for (let y = 0; y < CELLS; y++)
			for (let x = 0; x < CELLS; x++)
				if (0.55 + 0.3 * Math.cos(Math.atan2(y - 12, x - 12) - light) > threshold(x, y))
					g.fillRect(x, y, 1, 1);
		crater(g, [CELLS, CELLS], spot, 1, rays, { night, deep, cyan });
		g.restore();
	});
</script>

<canvas
	bind:this={canvas}
	width={CELLS}
	height={CELLS}
	class="shrink-0 rounded-full pixelated {invited
		? 'opacity-60 outline-1 -outline-offset-1 outline-edge outline-dashed'
		: ''}"
	style:width="{size}px"
	style:height="{size}px"
	aria-hidden="true"
></canvas>
