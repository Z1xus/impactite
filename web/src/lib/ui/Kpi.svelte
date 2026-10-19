<script lang="ts">
	import Num from "./Num.svelte";
	import Skeleton from "./Skeleton.svelte";

	let {
		label,
		value,
		change,
		format,
		active = false,
		unit = "%",
		before,
		pick
	}: {
		label: string;
		value: number | null;
		change: { change: number; good: boolean } | null;
		format: (value: number) => string;
		active?: boolean;
		unit?: string;
		before?: string;
		pick: () => void;
	} = $props();
</script>

<button
	type="button"
	class="flex flex-col gap-3 border-b px-4 py-4 text-left {active
		? 'border-cyan'
		: 'border-transparent hover:border-edge'}"
	title={before && `Previous period: ${before}`}
	onclick={pick}
>
	<span class="label {active ? 'text-cyan' : ''}">{label}</span>
	{#if value === null}
		<Skeleton class="h-[22px] w-20" />
	{:else}
		<Num {value} {format} class="text-num text-fg" />
	{/if}
	<span
		class="min-h-[11px] label {!change || change.change === 0
			? 'text-faint'
			: change.good
				? 'text-cyan'
				: 'text-bad'}"
	>
		{#if change}
			{change.change > 0 ? "+" : ""}{change.change}{unit}
		{/if}
	</span>
</button>
