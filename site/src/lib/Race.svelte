<script lang="ts">
	import Count from "./Count.svelte";
	import { reveal } from "./reveal.ts";

	let {
		rows,
		unit,
		digits = 0,
		better
	}: {
		rows: { label: string; value: number; ours?: boolean }[];
		unit: string;
		digits?: number;
		better: "more" | "less";
	} = $props();

	let on = $state(false);

	const max = $derived(Math.max(...rows.map((row) => row.value)));
	const best = $derived(
		better === "more"
			? Math.max(...rows.map((row) => row.value))
			: Math.min(...rows.map((row) => row.value))
	);
</script>

<ul class="flex flex-col gap-3" {@attach reveal((shown) => (on = shown))}>
	{#each rows as row, i (row.label)}
		<li class="grid grid-cols-[6.5rem_1fr_6.5rem] items-center gap-3">
			<span class="label {row.ours ? 'text-fg' : ''}">{row.label}</span>
			<span class="h-5 overflow-hidden rounded-px">
				<span
					class="block h-full min-w-px {on
						? 'w-(--width)'
						: 'w-0'} transition-[width] duration-[1200ms] ease-out motion-reduce:w-(--width) motion-reduce:transition-none {row.value ===
					best
						? 'bg-cyan'
						: 'dither [--dither-gap:var(--color-ink)]'}"
					style:--width="{(row.value / max) * 100}%"
					style:transition-delay="{i * 180}ms"
				></span>
			</span>
			<span
				class="text-right font-mono text-[14px] {row.value === best
					? 'text-cyan'
					: 'text-dim'}"
			>
				<Count value={row.value} {on} {digits} delay={i * 180} />
				<span class="text-[11px] text-faint">{unit}</span>
			</span>
		</li>
	{/each}
</ul>
