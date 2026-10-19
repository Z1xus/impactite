<script lang="ts">
	import { css } from "#lib/dither/bayer.ts";
	import { rock } from "#lib/dither/avatar.ts";
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
		const [raised, night, deep, cyan] = [
			"--color-raised",
			"--color-cyan-night",
			"--color-cyan-deep",
			"--color-cyan"
		].map(css);
		g.clearRect(0, 0, CELLS, CELLS);
		rock(
			g,
			CELLS,
			name,
			theme.shown === "light"
				? { floor: night, low: deep, high: cyan, rim: cyan }
				: { floor: raised, low: night, high: deep, rim: cyan }
		);
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
