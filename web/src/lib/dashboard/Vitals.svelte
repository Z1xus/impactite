<script lang="ts">
	import { untrack } from "svelte";
	import Chart from "#lib/dither/Chart.svelte";
	import type { Guide } from "#lib/dither/chart.ts";
	import { point } from "#lib/period.ts";
	import {
		VITALS,
		speed,
		toQueryFilters,
		vitalPages,
		vitalTrend,
		vitals,
		type Vital as Measured
	} from "#lib/stats.ts";
	import Empty from "#lib/ui/Empty.svelte";
	import Panel from "#lib/ui/Panel.svelte";
	import Rows from "#lib/ui/Rows.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";
	import Vital from "#lib/ui/Vital.svelte";
	import type { Context, PanelSpec } from "./types.ts";

	let {
		panel,
		ctx,
		arranging,
		hide,
		move
	}: {
		panel: PanelSpec;
		ctx: Context;
		arranging: boolean;
		hide: () => void;
		move: (step: number) => void;
	} = $props();

	type Ranked = NonNullable<Parameters<typeof Rows>[1]["rows"]>;
	type Detail = {
		metric: string;
		times: string[];
		values: number[];
		guides: Guide[];
		pages: Ranked;
	};

	let tab = $state("vitals");
	let metric = $state("lcp");
	let measured = $state.raw<Measured[] | null>(null);
	let detail = $state.raw<Detail | null>(null);
	let failed = $state(false);
	let tries = $state(0);

	// minutes have too few samples to rank, the last 30 minutes read as today by hour
	const grain = $derived(ctx.dates.grain === "time:minute" ? "time:hour" : ctx.dates.grain);
	const body = () => ({
		date_range: ctx.dates.current === "30m" ? "day" : ctx.dates.current,
		filters: toQueryFilters(ctx.filters)
	});
	const scope = $derived(
		JSON.stringify([ctx.domain, ctx.version, ctx.dates.current, ctx.filters, tries])
	);

	$effect(() => {
		void scope;
		return untrack(() => {
			let stale = false;
			failed = false;
			vitals(ctx.domain, body())
				.then((result) => {
					if (!stale) measured = result.vitals;
				})
				.catch(() => {
					if (!stale) failed = true;
				});
			return () => (stale = true);
		});
	});

	$effect(() => {
		void scope;
		const chosen = metric;
		const by = grain;
		return untrack(() => {
			let stale = false;
			Promise.all([
				vitalTrend(ctx.domain, { ...body(), metric: chosen, grain: by }),
				vitalPages(ctx.domain, { ...body(), metric: chosen })
			])
				.then(([trend, slowest]) => {
					if (stale) return;
					const values = trend.trend.map((item) => item.p75);
					detail = {
						metric: chosen,
						times: trend.trend.map((item) => item.time),
						values,
						// the poor line only shows once the chart reaches past good
						guides: [
							{ value: trend.good, tone: "warn" },
							...(Math.max(0, ...values) > trend.good
								? [{ value: trend.poor, tone: "bad" as const }]
								: [])
						],
						pages: slowest.pages.map((page) => ({
							key: page.page ?? "",
							label: page.page ?? "",
							value: page.p75 ?? 0,
							note: ctx.titles[page.page ?? ""],
							tone:
								page.rating === "poor"
									? "bad"
									: page.rating === "ni"
										? "warn"
										: undefined
						}))
					};
				})
				.catch(() => {
					if (!stale) failed = true;
				});
			return () => (stale = true);
		});
	});

	const shown = $derived(
		VITALS.map((item) => ({
			...item,
			value: measured?.find((vital) => vital.key === item.key)
		})).filter((item) => !measured || item.value?.samples)
	);
</script>

<Panel {arranging} fit {hide} {move} tabs={panel.tabs} bind:tab>
	{#if failed}
		<Empty retry={() => tries++} />
	{:else if measured === null}
		<div class="grid grid-cols-2 border-b border-line sm:grid-cols-3 lg:grid-cols-5">
			{#each VITALS as item (item.key)}
				<div class="flex flex-col gap-3 border-b border-transparent px-4 py-4">
					<span class="label">{item.label}</span>
					<Skeleton class="h-[22px] w-20" />
					<Skeleton class="h-2 w-full" />
				</div>
			{/each}
		</div>
		<div class="px-4 pt-4 pb-5"><Skeleton class="h-[220px] w-full" /></div>
	{:else if !shown.length}
		<Empty text="No page speed data yet." compact />
	{:else}
		<div class="grid grid-cols-2 border-b border-line sm:grid-cols-3 lg:grid-cols-5">
			{#each shown as item (item.key)}
				{#if item.value?.p75 !== undefined && item.value.rating && item.value.shares}
					<Vital
						label={item.label}
						title={item.title}
						value={speed(item.key, item.value.p75)}
						tone={item.value.rating}
						shares={item.value.shares}
						active={metric === item.key}
						pick={() => (metric = item.key)}
					/>
				{/if}
			{/each}
		</div>
		<div class="grid lg:grid-cols-[minmax(0,1fr)_24rem]">
			<div class="min-w-0 px-4 pt-4 pb-5">
				{#if !detail}
					<Skeleton class="h-[220px] w-full" />
				{:else if detail.values.length < 2}
					<Empty text="Not enough visits to draw a trend yet." compact />
				{:else}
					<Chart
						values={detail.values}
						labels={detail.times.map((time) => point(time, grain))}
						keys={detail.times}
						guides={detail.guides}
						format={(value) => speed(detail?.metric ?? metric, value)}
					/>
				{/if}
			</div>
			<div class="border-line py-2 max-lg:border-t lg:border-l">
				<Rows
					rows={detail?.pages ?? null}
					format={(value) => speed(detail?.metric ?? metric, value)}
					columns={["p75"]}
					pick={(page) => ctx.add("event:page", page)}
					filter
				/>
			</div>
		</div>
	{/if}
</Panel>
