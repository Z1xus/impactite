<script lang="ts">
	import { delta, format, METRICS } from "#lib/stats.ts";
	import Kpi from "./Kpi.svelte";

	let {
		totals,
		previous,
		active,
		warm,
		pick
	}: {
		totals: number[] | null;
		previous: number[] | null;
		active: number;
		warm?: (index: number) => void;
		pick: (index: number) => void;
	} = $props();
</script>

<div class="grid grid-cols-2 border-b border-line sm:grid-cols-3 lg:grid-cols-6">
	{#each METRICS as item, i (item.key)}
		<Kpi
			label={item.label}
			value={totals?.[i] ?? null}
			change={totals && previous ? delta(item.key, totals[i], previous[i]) : null}
			unit={item.key === "bounce_rate" ? "pp" : "%"}
			before={previous ? format(item.key, previous[i]) : undefined}
			format={(value) => format(item.key, value)}
			active={active === i}
			warm={() => warm?.(i)}
			pick={() => pick(i)}
		/>
	{/each}
</div>
