<script lang="ts">
	import { untrack } from "svelte";
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { api } from "#lib/api.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import { ICONIC, icon } from "#lib/icons/resolve.ts";
	import more from "pixelarticons/svg/more-vertical.svg?raw";
	import funnel from "pixelarticons/svg/filter.svg?raw";
	import sliders from "pixelarticons/svg/sliders.svg?raw";
	import { currentTeam, findSite, session } from "#lib/session.svelte.ts";
	import {
		METRICS,
		decodeFilter,
		encodeFilter,
		query,
		toQueryFilters,
		type Filter
	} from "#lib/stats.ts";
	import { current, interval, intervals, parse, previous } from "#lib/period.ts";
	import { downloadCsv } from "./csv.ts";
	import Filters from "./Filters.svelte";
	import Grid from "./Grid.svelte";
	import { createLayout } from "./layout.svelte.ts";
	import Overview from "./Overview.svelte";
	import type { Context, Funnel } from "./types.ts";
	import Details from "./Details.svelte";
	import FilterDialog from "./FilterDialog.svelte";
	import NewEvents from "#lib/settings/NewEvents.svelte";
	import Period from "./Period.svelte";
	import Waiting from "./Waiting.svelte";
	import Button from "#lib/ui/Button.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Menu from "#lib/ui/Menu.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Glyph from "#lib/ui/Glyph.svelte";
	import Title from "#lib/ui/Title.svelte";
	import Live from "#lib/ui/Live.svelte";
	import Select from "#lib/ui/Select.svelte";

	let {
		domain,
		timezone,
		readonly = false
	}: { domain: string; timezone: string; readonly?: boolean } = $props();

	// `team:<id>` shows every site of the team added up
	const combined = $derived(domain.startsWith("team:"));
	const editable = $derived(
		!readonly && ["owner", "admin", "editor"].includes(findSite(domain)?.role ?? "")
	);
	const team = $derived(currentTeam());
	const sites = $derived([
		...((team?.sites.length ?? 0) > 1 ? [{ key: "all", label: "All sites" }] : []),
		...(session.me?.teams.flatMap((item) => item.sites) ?? []).map((site) => ({
			key: site.domain,
			label: site.domain,
			glyph: icon("event:hostname", site.domain)
		}))
	]);

	const metric = $derived(
		METRICS.find((item) => item.key === page.url.searchParams.get("metric"))?.key ?? "visitors"
	);
	const filters = $derived(
		page.url.searchParams
			.getAll("f")
			.map(decodeFilter)
			.filter((filter): filter is Filter => filter !== null)
	);
	const view = $derived(parse(page.url.searchParams, timezone));
	const dates = $derived.by(() => {
		if (!view) return null;
		const range = current(view);
		return {
			current: range,
			previous: previous(view, range),
			overlay: view.compare !== "off",
			interval: interval(view, range),
			grain: `time:${interval(view, range)}`,
			choices: intervals(range)
		};
	});
	const imports = $derived(page.url.searchParams.get("imports") !== "0");
	const realtime = $derived(view?.period === "realtime");
	const converting = $derived(filters.some((filter) => filter.dim === "event:goal"));

	const layout = createLayout(
		() => domain,
		() => readonly
	);
	let empty = $state<boolean | null>(null);
	let version = $state(0);
	let hasImports = $state(false);
	let editing = $state<Filter | null | undefined>(undefined);
	let details = $state<{ dim: string; title: string } | null>(null);
	let reviewing = $state(false);
	let newEvents = $state<{ name: string; count: number; revenue: boolean }[]>([]);
	let goals = $state<string[]>([]);
	let revenue = $state(false);
	let funnels = $state<Funnel[]>([]);
	let titles = $state<Record<string, string>>({});
	let propKeys = $state<string[]>([]);

	function navigate(change: (params: URLSearchParams) => void) {
		const params = new URLSearchParams(page.url.search);
		change(params);
		// the current path is already resolved, under /[site] or /share/[slug]
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		goto(`${page.url.pathname}?${params}`, { reset: false });
	}

	const go = (change: Record<string, string | null>) =>
		navigate((params) => {
			for (const [key, value] of Object.entries(change))
				if (value === null) params.delete(key);
				else params.set(key, value);
		});

	// the filters in the url, replaced as a whole
	const setRaw = (raw: string[]) =>
		navigate((params) => {
			params.delete("f");
			for (const item of raw) params.append("f", item);
		});

	function setFilter(filter: Filter) {
		editing = undefined;
		setRaw([
			...filters.filter((item) => item.dim !== filter.dim).map(encodeFilter),
			encodeFilter(filter)
		]);
	}

	function removeFilter(dim: string) {
		editing = undefined;
		setRaw(filters.filter((item) => item.dim !== dim).map(encodeFilter));
	}

	const ctx = $derived<Context | null>(
		dates && {
			domain,
			combined,
			dates,
			filters,
			imports,
			version,
			converting,
			titles,
			goals,
			propKeys,
			funnels,
			add: (dim, value) => setFilter({ op: "is", dim, values: [value] }),
			expand: (dim, title) => (details = { dim, title })
		}
	);

	$effect(() => {
		if (!realtime) return;
		const timer = setInterval(() => version++, 30_000);
		return () => clearInterval(timer);
	});

	$effect(() => {
		if (readonly || combined) {
			empty = false;
			return;
		}
		let stale = false;
		query(domain, { metrics: ["events"], date_range: "all" }).then((result) => {
			if (!stale) empty = !result.results[0]?.metrics[0];
		});
		return () => (stale = true);
	});

	$effect(() => {
		if (!editable || combined) return;
		api<{ events: typeof newEvents }>(`/api/app/sites/${domain}/goals/suggestions`)
			.then((result) => (newEvents = result.events))
			.catch(() => (newEvents = []));
	});

	function loadGoals() {
		api<{ goals: { name: string; currency: string }[] }>(`/api/app/sites/${domain}/goals`)
			.then((result) => {
				goals = result.goals.map((goal) => goal.name);
				revenue = result.goals.some((goal) => goal.currency);
			})
			.catch(() => (goals = []));
	}

	// the all sites view has no goals, funnels or titles of its own
	$effect(() => {
		if (combined) return;
		api<Record<string, string>>(`/api/app/sites/${domain}/titles`)
			.then((result) => (titles = result))
			.catch(() => (titles = {}));
		api<{ funnels: Funnel[] }>(`/api/app/sites/${domain}/funnels`)
			.then((result) => (funnels = result.funnels))
			.catch(() => (funnels = []));
		untrack(loadGoals);
	});

	$effect(() => {
		void version;
		if (!dates || empty !== false) return;
		const range = dates.current === "30m" ? "day" : dates.current;
		api<string[]>("/api/app/props", { site_id: domain, date_range: range }).then(
			(keys) => (propKeys = keys)
		);
	});
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && (layout.arranging = false)} />

<Dialog
	title={editing ? "edit filter" : "filter"}
	bind:open={
		() => editing !== undefined,
		(open) => {
			if (!open) editing = undefined;
		}
	}
>
	{#if dates}
		<FilterDialog
			{domain}
			range={dates.current}
			{goals}
			{propKeys}
			initial={editing ?? null}
			apply={setFilter}
			remove={removeFilter}
		/>
	{/if}
</Dialog>

<Dialog title="new events" bind:open={reviewing}>
	<NewEvents
		{domain}
		bind:events={newEvents}
		changed={() => {
			loadGoals();
			if (newEvents.length === 0) reviewing = false;
		}}
	/>
</Dialog>

<Dialog
	wide
	title={details?.title ?? ""}
	bind:open={
		() => details !== null,
		(open) => {
			if (!open) details = null;
		}
	}
>
	{#if details && dates}
		<Details
			{domain}
			dim={details.dim}
			range={dates.current}
			filters={toQueryFilters(filters)}
			{imports}
			{converting}
			{revenue}
			iconic={ICONIC.has(details.dim)}
			pick={(value) => {
				const dim = details?.dim ?? "";
				details = null;
				ctx?.add(dim, value);
			}}
		/>
	{/if}
</Dialog>

<div class="flex flex-col gap-6">
	<div class="flex flex-wrap items-center gap-3">
		{#if readonly}
			<Title>
				<Glyph spec={icon("event:hostname", domain)} fallback={domain} />
				{domain}
			</Title>
		{:else}
			<Select
				value={combined ? "all" : domain}
				options={sites}
				data
				plain
				pick={(key) =>
					goto(key === "all" ? resolve("/all") : resolve("/[site]", { site: key }))}
			>
				{#snippet footer()}
					{#if editable}
						<a
							href={resolve("/[site]/settings/[[section]]", { site: domain })}
							class="flex h-9 items-center gap-2 px-3 text-[14px] text-dim hover:bg-line hover:text-fg"
							><Icon svg={sliders} size={14} />Site settings</a
						>
					{/if}
				{/snippet}
			</Select>
		{/if}
		{#if empty === false}<Live site={domain} />{/if}
		<div class="ml-auto flex items-center gap-2 max-sm:w-full max-sm:justify-between">
			{#if layout.arranging}
				<Button variant="quiet" class="ml-auto" onclick={() => (layout.arranging = false)}
					>Done</Button
				>
			{:else if empty === false}
				<Button variant="text" onclick={() => (editing = null)}
					><Icon svg={funnel} size={14} />Filter</Button
				>
				{#if view}
					<Period {view} {go} />
					{#if !readonly}
						<Menu
							label="Dashboard options"
							icon={more}
							plain
							iconOnly
							align="right"
							items={[
								{ label: "Edit layout", onclick: () => (layout.arranging = true) },
								...(hasImports
									? [
											{
												label: "Include imported data",
												current: imports,
												onclick: () => go({ imports: imports ? "0" : null })
											}
										]
									: []),
								"divider",
								{
									label: "Download CSV",
									onclick: () => ctx && attempt(() => downloadCsv(ctx))
								}
							]}
						/>
					{/if}
				{/if}
			{/if}
		</div>
	</div>

	<Filters
		{domain}
		{filters}
		segmented={!readonly && !combined && empty === false}
		{editable}
		edit={(filter) => (editing = filter)}
		remove={removeFilter}
		apply={setRaw}
	/>

	{#if editable && !combined && empty === false && newEvents.length}
		<div
			class="flex flex-wrap items-center justify-between gap-4 rounded-px border border-line bg-panel py-2 pr-2 pl-4"
		>
			<span class="text-[14px] text-dim">
				{newEvents.length === 1
					? `Your site sends ${newEvents[0].name} events, but there's no goal for them yet.`
					: `Your site sends ${newEvents.length} kinds of events that have no goal yet.`}
			</span>
			<Button variant="quiet" onclick={() => (reviewing = true)}>Review</Button>
		</div>
	{/if}

	{#if empty && !combined}
		<Waiting
			{domain}
			landed={() => {
				empty = false;
				version++;
			}}
		/>
	{:else if empty === false && ctx}
		<Overview
			{ctx}
			metric={realtime ? "pageviews" : metric}
			{realtime}
			table={layout.table}
			found={(has) => (hasImports = has)}
			pick={(next) => navigate((params) => params.set("metric", next))}
			setTable={layout.setTable}
			setGrain={(grain) => go({ interval: grain })}
		/>
		<Grid {ctx} {layout} />
	{/if}
</div>
