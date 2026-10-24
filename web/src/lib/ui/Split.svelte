<script lang="ts">
	let { shares, labels }: { shares: number[]; labels: string[] } = $props();

	const FILLS = ["var(--color-cyan-deep)", "var(--color-warn)", "var(--color-bad)"];
</script>

<span class="flex h-2 min-w-0 flex-1 gap-0.5" role="img" aria-label={labels.join(", ")}>
	{#each shares as share, i (i)}
		{#if share > 0}
			<span
				class="h-full rounded-px dither transition-[flex-grow] duration-300 ease-out motion-reduce:transition-none"
				style:flex-grow={share}
				style:--dither-fill={FILLS[i]}
				title={labels[i]}
			></span>
		{/if}
	{/each}
</span>
