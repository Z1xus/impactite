<script lang="ts">
	import { paths } from "./pixels.ts";

	let { pixels, width, palette }: { pixels: Uint8Array; width: number; palette: string[] } =
		$props();
</script>

<svg
	class="absolute inset-0 h-full w-full"
	viewBox="0 0 {width} {pixels.length / width}"
	preserveAspectRatio="none"
	shape-rendering="crispEdges"
	aria-hidden="true"
>
	{#each paths(pixels, width, palette.length) as d, i (i)}
		<path {d} fill={palette[i]} />
	{/each}
</svg>
