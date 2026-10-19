<script lang="ts">
	import { untrack } from "svelte";
	import Chart from "#lib/dither/Chart.svelte";
	import { clock } from "#lib/display.svelte.ts";
	import { METRICS, format, query, toQueryFilters, type Metric } from "#lib/stats.ts";
	import Kpis from "#lib/ui/Kpis.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Table from "#lib/ui/Table.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import type { Context } from "./types.ts";

	let {
		ctx,
		metric,
		realtime,
		table,
		found,
		pick,
		setTable,
		setGrain
	}: {
		ctx: Context;
		metric: Metric;
		realtime: boolean;
		table: boolean;
		found: (hasImports: boolean) => void;
		pick: (metric: Metric) => void;
		setTable: (table: boolean) => void;
		setGrain: (interval: string) => void;
	} = $props();

	type Line = { key: string; label: string; values: number[] };

	const KEYS = METRICS.map((item) => item.key);

	let totals = $state.raw<number[] | null>(null);
	let previousTotals = $state.raw<number[] | null>(null);
	let series = $state.raw<number[]>([]);
	let previousSeries = $state.raw<number[]>([]);
	let lines = $state.raw<Line[]>([]);
	let sort = $state<[string, "asc" | "desc"]>(["", "asc"]);

	const labels = $derived(lines.map((line) => line.label));
	const sorted = $derived.by(() => {
		const at = KEYS.indexOf(sort[0] as Metric);
		if (at < 0) return lines;
		const sign = sort[1] === "desc" ? -1 : 1;
		return [...lines].sort((a, b) => sign * (a.values[at] - b.values[at]));
	});
	// realtime minutes only count pageviews
	const columns = $derived(
		realtime ? METRICS.filter((item) => item.key === "pageviews") : METRICS
	);

	function label(raw: string, grain: string) {
		if (grain === "time:hour" || grain === "time:minute") return clock(raw.slice(11, 16));
		const date = new Date(raw.replace(" ", "T") + (raw.length === 10 ? "T00:00:00" : ""));
		return date.toLocaleDateString(
			"en-US",
			grain === "time:month"
				? { month: "short", year: "2-digit" }
				: { month: "short", day: "numeric" }
		);
	}

	const key = $derived(
		JSON.stringify([
			ctx.domain,
			ctx.version,
			ctx.dates,
			ctx.filters,
			ctx.imports,
			table,
			metric
		])
	);

	function load() {
		const { dates, domain, imports } = ctx;
		const where = toQueryFilters(ctx.filters);
		const ask = (body: Record<string, unknown>) => query(domain, body, imports);
		const { previous, grain } = dates;
		let stale = false;
		Promise.all([
			ask({ metrics: KEYS, date_range: dates.current, filters: where }),
			previous && ask({ metrics: KEYS, date_range: previous, filters: where }),
			ask({
				metrics: table ? KEYS : [metric],
				date_range: dates.current,
				dimensions: [grain],
				filters: where
			}),
			previous &&
				dates.overlay &&
				ask({
					metrics: [metric],
					date_range: previous,
					dimensions: [grain],
					filters: where
				})
		]).then(([now, before, line, previousLine]) => {
			if (stale) return;
			found(!imports || (now.meta?.imports_skip_reason ?? "") !== "no_imported_data");
			totals = now.results[0]?.metrics ?? null;
			previousTotals = before ? (before.results[0]?.metrics ?? null) : null;
			const at = table ? KEYS.indexOf(metric) : 0;
			series = line.results.map((row) => row.metrics[Math.max(at, 0)]);
			lines = line.results.map((row) => ({
				key: row.dimensions[0],
				label: label(row.dimensions[0], grain),
				values: row.metrics
			}));
			previousSeries = previousLine ? previousLine.results.map((row) => row.metrics[0]) : [];
		});
		return () => (stale = true);
	}

	$effect(() => {
		void key;
		return untrack(load);
	});
</script>

{#snippet controls()}
	{#if ctx.dates.choices.length > 1}
		<Select
			align="left"
			plain
			value={ctx.dates.interval}
			options={ctx.dates.choices.map((key) => ({ key, label: key }))}
			pick={setGrain}
		/>
	{/if}
	<Toggle
		plain
		bind:value={
			() => (table ? "table" : "chart"),
			(view: "chart" | "table") => setTable(view === "table")
		}
		options={[
			{ key: "chart", label: "Chart" },
			{ key: "table", label: "Table" }
		]}
	/>
{/snippet}

<section class="rounded-px border border-line bg-panel">
	<Kpis
		{totals}
		previous={previousTotals}
		active={KEYS.indexOf(metric)}
		pick={(i) => pick(KEYS[i])}
	/>
	<div class="px-4 pt-3 pb-5">
		{#if table}
			<div class="relative">
				<div class="absolute top-0 left-0 z-20 flex h-control items-center gap-2">
					{@render controls()}
				</div>
				<div
					class="max-h-96 [scrollbar-gutter:stable] overflow-y-auto pr-2 [&_thead_th]:sticky [&_thead_th]:top-0 [&_thead_th]:z-10 [&_thead_th]:h-9 [&_thead_th]:bg-panel [&_thead_th]:shadow-[inset_0_-1px_0_var(--color-line)] [&_thead_th:first-child]:min-w-56 [&>div]:overflow-visible"
				>
					<Table
						columns={columns.map((item) => ({
							key: item.key,
							label: item.label,
							format: (value: number | null) => format(item.key, value ?? 0)
						}))}
						rows={sorted.map((line) => ({
							...line,
							values: columns.map((item) => line.values[KEYS.indexOf(item.key)])
						}))}
						bind:sort
					/>
				</div>
			</div>
		{:else}
			<div class="mb-2 flex items-center gap-2">
				{@render controls()}
			</div>
			<Chart
				values={series}
				previous={previousSeries}
				{labels}
				keys={lines.map((line) => line.key)}
				format={(value) => format(metric, value)}
			/>
		{/if}
	</div>
</section>
