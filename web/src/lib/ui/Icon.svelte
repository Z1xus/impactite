<script lang="ts">
	let {
		svg,
		size = 16,
		class: className = ""
	}: { svg: string; size?: number; class?: string } = $props();

	const paths = $derived([...svg.matchAll(/ d="([^"]+)"/g)].map((match) => match[1]));
</script>

<svg
	class="shrink-0 {className}"
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="currentColor"
	aria-hidden="true"
>
	{#each paths as d, i (i)}
		<path {d} />
	{/each}
</svg>
