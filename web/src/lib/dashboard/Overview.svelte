<script lang="ts">
	import { untrack } from "svelte";
	import Chart from "#lib/dither/Chart.svelte";
	import type { Band, Mark } from "#lib/dither/chart.ts";
	import { clock } from "#lib/display.svelte.ts";
	import { METRICS, cached, format, toQueryFilters, type Metric } from "#lib/stats.ts";
	import Empty from "#lib/ui/Empty.svelte";
	import Kpis from "#lib/ui/Kpis.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Table from "#lib/ui/Table.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import { local, missed, span } from "#lib/uptime/format.ts";
	import type { SiteIncident } from "#lib/uptime/site.svelte.ts";
	import type { Context, Note } from "./types.ts";

	let {
		ctx,
		metric,
		realtime,
		incidents = [],
		notes = [],
		note,
		table,
		found,
		pick,
		setTable,
		setGrain
	}: {
		ctx: Context;
		metric: Metric;
		realtime: boolean;
		incidents?: SiteIncident[];
		notes?: Note[];
		note?: (day: string) => void;
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
	let failed = $state(false);
	let tries = $state(0);
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

	// buckets are named in the site's zone, so local incident times sort against them as text
	const bands = $derived.by((): Band[] => {
		const keys = lines.map((line) => line.key);
		const last = keys.length - 1;
		return incidents.flatMap((incident) => {
			if (!keys.length || incident.start > `${keys[last]}\uffff`) return [];
			if (incident.end !== null && incident.end < keys[0]) return [];
			const at = (time: string) =>
				Math.max(
					0,
					keys.findLastIndex((key) => key <= time)
				);
			const notes = [
				local(incident.start),
				incident.ended_at ? span(incident.duration) : "ongoing"
			];
			if (incident.visits_missed) notes.push(missed(incident.visits_missed));
			return [
				{
					from: at(incident.start),
					to: incident.end === null ? last : at(incident.end),
					label: incident.cause,
					notes
				}
			];
		});
	});

	const hourly = $derived(ctx.dates.grain === "time:hour" || ctx.dates.grain === "time:minute");
	// a note sits on the bucket its day falls in, or the first hour of that day
	const place = (keys: string[], day: string) =>
		hourly
			? keys.findIndex((key) => key.startsWith(day))
			: day > keys[keys.length - 1] || day < keys[0].slice(0, 10)
				? -1
				: keys.findLastIndex((key) => key.slice(0, 10) <= day);

	const marks = $derived.by((): Mark[] => {
		const keys = lines.map((line) => line.key);
		if (!keys.length) return [];
		const placed: Record<number, string[]> = {};
		for (const item of notes) {
			const at = place(keys, item.day);
			if (at >= 0) (placed[at] ??= []).push(item.text);
		}
		return Object.entries(placed).map(([at, lines]) => ({
			at: Number(at),
			tone: "note",
			lines
		}));
	});

	const key = $derived(
		JSON.stringify([
			ctx.domain,
			ctx.version,
			ctx.dates,
			ctx.filters,
			ctx.imports,
			table,
			metric,
			tries
		])
	);

	const ask = (body: Record<string, unknown>) => cached(ctx.domain, body, ctx.imports);

	function ahead(at: number) {
		const { current, previous, grain, overlay } = ctx.dates;
		const where = toQueryFilters(ctx.filters);
		const body = { metrics: [KEYS[at]], dimensions: [grain], filters: where };
		if (!table) ask({ ...body, date_range: current }).catch(() => {});
		if (!table && previous && overlay) ask({ ...body, date_range: previous }).catch(() => {});
	}

	function load() {
		const { dates, imports } = ctx;
		const where = toQueryFilters(ctx.filters);
		const { previous, grain } = dates;
		let stale = false;
		failed = false;
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
		])
			.then(([now, before, line, previousLine]) => {
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
				previousSeries = previousLine
					? previousLine.results.map((row) => row.metrics[0])
					: [];
			})
			.catch(() => {
				if (!stale) failed = true;
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
			value={ctx.dates.interval}
			options={ctx.dates.choices.map((key) => ({ key, label: key }))}
			pick={setGrain}
		/>
	{/if}
	<Toggle
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
	{#if failed}
		<Empty retry={() => tries++} />
	{:else}
		<Kpis
			{totals}
			previous={previousTotals}
			active={KEYS.indexOf(metric)}
			warm={ahead}
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
					{bands}
					{marks}
					mark={note && {
						label: "click to add a note",
						pick: (at) => note(lines[at].key.slice(0, 10))
					}}
					format={(value) => format(metric, value)}
				/>
			{/if}
		</div>
	{/if}
</section>
