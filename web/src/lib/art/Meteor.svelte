<script lang="ts">
	import Pixels from "#lib/dither/Pixels.svelte";
	import { paint } from "#lib/dither/pixels.ts";
	import { meteor, normalize, LIGHT, PALETTE } from "./meteor.ts";

	let { size = 192, pixel = 3 }: { size?: number; pixel?: number } = $props();

	const FPS = 20;
	const n = $derived(Math.round(size / pixel));
	let ready = $state(false);
	let light = LIGHT;
	let aim = LIGHT;

	let canvas: HTMLCanvasElement;

	$effect(() => {
		canvas.width = n;
		canvas.height = n;
		const ctx = canvas.getContext("2d");
		if (!ctx) return;
		const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
		let frame = 0;
		let last = 0;
		const tick = (now: number) => {
			if (now - last >= 1000 / FPS) {
				last = now;
				light = normalize(light.map((v, k) => v + (aim[k] - v) * 0.2));
				paint(ctx, meteor(n, now * 0.00009, light), n, PALETTE);
			}
			frame = requestAnimationFrame(tick);
		};
		const point = (event: PointerEvent) => {
			if (event.pointerType !== "mouse") return;
			const box = canvas.getBoundingClientRect();
			const reach = Math.max(box.width, 240) * 1.5;
			const dx = Math.max(
				-1,
				Math.min(1, (event.clientX - box.left - box.width / 2) / reach)
			);
			const dy = Math.max(
				-1,
				Math.min(1, (event.clientY - box.top - box.height / 2) / reach)
			);
			aim = normalize([dx, dy, 0.55]);
		};
		paint(ctx, meteor(n, 0.8), n, PALETTE);
		ready = true;
		if (!still) {
			frame = requestAnimationFrame(tick);
			addEventListener("pointermove", point);
		}
		return () => {
			cancelAnimationFrame(frame);
			removeEventListener("pointermove", point);
		};
	});
</script>

<div
	class="relative pixelated"
	style:width="{size}px"
	style:max-width="100%"
	style:aspect-ratio="1"
	aria-hidden="true"
>
	{#if !ready}
		<Pixels
			pixels={meteor(n, 0.8)}
			width={n}
			palette={PALETTE.map(([r, g, b]) => `rgb(${r} ${g} ${b})`)}
		/>
	{/if}
	<canvas bind:this={canvas} width={n} height={n} class="absolute inset-0 h-full w-full"></canvas>
</div>
