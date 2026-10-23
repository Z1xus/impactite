<script lang="ts">
	import Kpi from "#lib/ui/Kpi.svelte";
	import { tick } from "./clock.svelte.ts";
	import { latency, percent, span } from "./format.ts";
	import type { Detail } from "./types.ts";

	let { detail }: { detail: Detail | null } = $props();

	const kpis = $derived(detail?.kpis);
	const beat = $derived(detail?.monitor.kind === "heartbeat");
	const days = $derived(
		kpis?.cert_expires ? Math.ceil((kpis.cert_expires - tick.now) / 86_400) : null
	);
	const figure = (value: number | null | undefined) => ({
		value: detail ? (value ?? 0) : null,
		blank: detail && value == null ? "–" : undefined
	});
</script>

<div class="grid grid-cols-[repeat(auto-fit,minmax(9rem,1fr))] border-b border-line">
	<Kpi label="uptime" change={null} format={percent} {...figure(kpis?.uptime)} />
	<Kpi label="downtime" change={null} format={span} {...figure(kpis?.downtime)} />
	<Kpi
		label="incidents"
		change={null}
		format={(value) => String(Math.round(value))}
		{...figure(kpis?.incidents)}
	/>
	{#if !beat}
		<Kpi label="response" change={null} format={latency} {...figure(kpis?.p50)} />
		<Kpi label="slowest 5%" change={null} format={latency} {...figure(kpis?.p95)} />
	{/if}
	{#if days !== null}
		<Kpi
			label="certificate"
			change={null}
			value={days}
			low={days <= 14}
			format={(value) => `${Math.round(value)} days`}
		/>
	{/if}
</div>
