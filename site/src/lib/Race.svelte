<script lang="ts">
	import Count from "./Count.svelte";

	let {
		rows,
		on,
		unit
	}: {
		rows: { label: string; value: number; ours?: boolean }[];
		on: boolean;
		unit: string;
	} = $props();

	const max = $derived(Math.max(...rows.map((row) => row.value)));
</script>

<ul class="flex flex-col gap-3">
	{#each rows as row, i (row.label)}
		<li class="grid grid-cols-[7.5rem_1fr_5.5rem] items-center gap-4">
			<span class="label {row.ours ? 'text-cyan' : ''}">{row.label}</span>
			<span class="h-6 overflow-hidden rounded-px">
				<span
					class="block h-full transition-[width] duration-[1200ms] ease-[steps(14)] {row.ours
						? 'bg-cyan'
						: 'dither [--dither-gap:var(--color-ink)]'}"
					style:width={on ? `${(row.value / max) * 100}%` : "0%"}
					style:transition-delay="{i * 180}ms"
				></span>
			</span>
			<span class="text-right font-mono text-[15px] {row.ours ? 'text-fg' : 'text-dim'}">
				<Count value={row.value} {on} delay={i * 180} />
				<span class="text-[12px] text-faint">{unit}</span>
			</span>
		</li>
	{/each}
</ul>
