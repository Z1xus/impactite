<script lang="ts">
	import { untrack } from "svelte";
	import {
		VITALS,
		speed,
		toQueryFilters,
		vitalPages,
		vitals,
		type Vital as Measured
	} from "#lib/stats.ts";
	import Empty from "#lib/ui/Empty.svelte";
	import Panel from "#lib/ui/Panel.svelte";
	import Rows from "#lib/ui/Rows.svelte";
	import Select from "#lib/ui/Select.svelte";
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

	type Ranked = Parameters<typeof Rows>[1]["rows"];

	let tab = $state("vitals");
	let metric = $state("lcp");
	let measured = $state.raw<Measured[] | null>(null);
	let pages = $state.raw<Ranked>(null);
	let loading = $state(false);
	let failed = $state(false);
	let tries = $state(0);

	const body = () => ({
		date_range: ctx.dates.current === "30m" ? "day" : ctx.dates.current,
		filters: toQueryFilters(ctx.filters)
	});
	const key = $derived(
		JSON.stringify([
			ctx.domain,
			ctx.version,
			ctx.dates.current,
			ctx.filters,
			tab,
			metric,
			tries
		])
	);

	function load() {
		let stale = false;
		loading = true;
		failed = false;
		const request =
			tab === "vitals"
				? vitals(ctx.domain, body()).then((result) => {
						if (!stale) measured = result.vitals;
					})
				: vitalPages(ctx.domain, { ...body(), metric }).then((result) => {
						if (stale) return;
						pages = result.pages.map((page) => ({
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
						}));
					});
		request
			.then(() => {
				if (!stale) loading = false;
			})
			.catch(() => {
				if (stale) return;
				failed = true;
				loading = false;
			});
		return () => (stale = true);
	}

	$effect(() => {
		void key;
		return untrack(load);
	});

	const shown = $derived(
		VITALS.map((item) => ({
			...item,
			value: measured?.find((vital) => vital.key === item.key)
		})).filter((item) => !measured || item.value?.samples)
	);
</script>

{#snippet picker()}
	<Select
		align="right"
		value={metric}
		options={VITALS.map((item) => ({ key: item.key, label: item.label }))}
		data
		pick={(item) => (metric = item)}
	/>
{/snippet}

<Panel
	{arranging}
	fit
	{hide}
	{move}
	tabs={panel.tabs}
	warm={(next) => {
		if (next === "pages") vitalPages(ctx.domain, { ...body(), metric }).catch(() => {});
	}}
	bind:tab
	actions={tab === "pages" ? picker : undefined}
>
	{#if failed}
		<Empty retry={() => tries++} />
	{:else if tab === "pages"}
		<Rows
			rows={pages}
			{loading}
			format={(value) => speed(metric, value)}
			columns={["p75"]}
			pick={(page) => ctx.add("event:page", page)}
		/>
	{:else if measured === null}
		<div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5">
			{#each VITALS as item (item.key)}
				<div class="flex flex-col gap-3 px-4 py-4">
					<span class="label">{item.label}</span>
					<Skeleton class="h-[22px] w-20" />
					<Skeleton class="h-2 w-full" />
				</div>
			{/each}
		</div>
	{:else if !shown.length}
		<Empty text="No page speed yet." compact />
	{:else}
		<div
			class="grid grid-cols-2 transition-opacity duration-150 sm:grid-cols-3 lg:grid-cols-5 {loading
				? 'opacity-60 delay-300'
				: ''}"
		>
			{#each shown as item (item.key)}
				{#if item.value?.p75 !== undefined && item.value.rating && item.value.shares}
					<Vital
						label={item.label}
						title={item.title}
						value={speed(item.key, item.value.p75)}
						tone={item.value.rating}
						shares={item.value.shares}
					/>
				{/if}
			{/each}
		</div>
	{/if}
</Panel>
