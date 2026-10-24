<script lang="ts">
	import { css, threshold } from "#lib/dither/bayer.ts";

	let { down = false }: { down?: boolean } = $props();

	const ROWS = 10;
	const CELL = 4;

	let canvas: HTMLCanvasElement;

	// down dithers from the raised surface back to the page
	$effect(() => {
		const g = canvas.getContext("2d");
		if (!g) return;
		const [ink, raised] = ["--color-ink", "--color-raised"].map(css);
		const draw = () => {
			const w = Math.ceil(canvas.clientWidth / CELL);
			canvas.width = w;
			canvas.height = ROWS;
			g.fillStyle = down ? raised : ink;
			g.fillRect(0, 0, w, ROWS);
			g.fillStyle = down ? ink : raised;
			for (let y = 0; y < ROWS; y++)
				for (let x = 0; x < w; x++)
					if ((y + 0.5) / ROWS > threshold(x, y)) g.fillRect(x, y, 1, 1);
		};
		const observer = new ResizeObserver(draw);
		observer.observe(canvas);
		return () => observer.disconnect();
	});
</script>

<canvas
	bind:this={canvas}
	class="block w-full pixelated"
	style:height="{ROWS * CELL}px"
	aria-hidden="true"
></canvas>
