<script lang="ts">
	import { pixelize } from "#lib/icons/pixelize.ts";
	import type { Spec } from "#lib/icons/resolve.ts";

	let { spec, fallback }: { spec: Spec | null; fallback?: string } = $props();

	const CELLS = 16;

	let canvas = $state<HTMLCanvasElement>();
	let status = $state<"pending" | "ready" | "missing">("pending");
	let size = $state(16);

	$effect(() => {
		const ratio = devicePixelRatio || 1;
		size = (Math.max(1, Math.round((16 * ratio) / CELLS)) * CELLS) / ratio;
		status = spec ? "pending" : "missing";
		if (!spec) return;
		let stale = false;
		pixelize(spec, CELLS, "true").then((image) => {
			if (stale) return;
			if (!image || !canvas) {
				status = "missing";
				return;
			}
			canvas.getContext("2d")?.putImageData(image, 0, 0);
			status = "ready";
		});
		return () => (stale = true);
	});
</script>

<span class="grid h-4 w-4 shrink-0 place-items-center" aria-hidden="true">
	{#if status === "missing"}
		{#if fallback}
			<span
				class="grid h-4 w-4 place-items-center rounded-[2px] bg-line font-mono text-[11px] leading-none text-dim"
			>
				{fallback.slice(0, 1).toUpperCase()}
			</span>
		{/if}
	{:else}
		<canvas
			bind:this={canvas}
			width={CELLS}
			height={CELLS}
			class="transition-opacity duration-150 pixelated {status === 'ready'
				? 'opacity-100'
				: 'opacity-0'}"
			style:width="{size}px"
			style:height="{size}px"
		></canvas>
	{/if}
</span>
