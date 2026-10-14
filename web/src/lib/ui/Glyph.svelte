<script lang="ts">
	import { render, type Spec } from "#lib/icons/lens.ts";

	let { spec, fallback }: { spec: Spec | null; fallback?: string } = $props();

	let canvas: HTMLCanvasElement;
	let ready = $state(false);

	$effect(() => {
		const letter: Spec | null = fallback ? { kind: "letter", text: fallback } : null;
		let stale = false;
		ready = false;
		(async () => {
			const image = (spec && (await render(spec))) ?? (letter && (await render(letter)));
			if (stale || !image) return;
			canvas.getContext("2d")?.putImageData(image, 0, 0);
			ready = true;
		})();
		return () => (stale = true);
	});
</script>

<canvas
	bind:this={canvas}
	width="16"
	height="16"
	class="h-4 w-4 shrink-0 transition-opacity duration-150 pixelated {ready
		? 'opacity-100'
		: 'opacity-0'}"
	aria-hidden="true"
></canvas>
