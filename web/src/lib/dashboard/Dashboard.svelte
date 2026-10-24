<script lang="ts">
	import { tick, untrack } from "svelte";
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { get } from "#lib/api.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import { ICONIC, icon } from "#lib/icons/resolve.ts";
	import more from "pixelarticons/svg/more-vertical.svg?raw";
	import funnel from "pixelarticons/svg/filter.svg?raw";
	import sliders from "pixelarticons/svg/sliders.svg?raw";
	import { display } from "#lib/display.svelte.ts";
	import { currentTeam, findSite, session, teamTimezone } from "#lib/session.svelte.ts";
	import {
		METRICS,
		cached,
		propKeys,
		newEvents,
		decodeFilter,
		encodeFilter,
		toQueryFilters,
		type Filter,
		type NewEvent
	} from "#lib/stats.ts";
	import { current, interval, intervals, parse, previous } from "#lib/period.ts";
	import { downloadCsv } from "./csv.ts";
	import Filters from "./Filters.svelte";
	import Grid from "./Grid.svelte";
	import { createLayout } from "./layout.svelte.ts";
	import Overview from "./Overview.svelte";
	import type { Context, Funnel, Note } from "./types.ts";
	import Notes from "./Notes.svelte";
	import Details from "./Details.svelte";
	import FilterDialog from "./FilterDialog.svelte";
	import NewEvents from "#lib/settings/NewEvents.svelte";
	import Period from "./Period.svelte";
	import Loading from "./Loading.svelte";
	import Waiting from "./Waiting.svelte";
	import { warm } from "./warm.ts";
	import Banner from "#lib/ui/Banner.svelte";
	import Button from "#lib/ui/Button.svelte";
	import Credit from "#lib/ui/Credit.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Menu from "#lib/ui/Menu.svelte";
	import Empty from "#lib/ui/Empty.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Glyph from "#lib/ui/Glyph.svelte";
	import Title from "#lib/ui/Title.svelte";
	import Live from "#lib/ui/Live.svelte";
	import State from "#lib/ui/State.svelte";
	import { siteUptime } from "#lib/uptime/site.svelte.ts";
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
		...((team?.sites.length ?? 0) > 1 && display.all === "show"
			? [{ key: "all", label: "All sites" }]
			: []),
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

	const uptime = siteUptime(
		() => domain,
		() => dates?.current ?? null,
		() => !readonly && !combined && !!findSite(domain),
		() => team?.id ?? null
	);

	const layout = createLayout(
		() => domain,
		() => readonly
	);
	let empty = $state<boolean | null>(null);
	let failed = $state(false);
	let checks = $state(0);
	let version = $state(0);
	let hasImports = $state(false);
	let editing = $state<Filter | null | undefined>(undefined);
	let details = $state<{ dim: string; title: string } | null>(null);
	let reviewing = $state(false);
	let unlisted = $state.raw<NewEvent[]>([]);
	let goals = $state.raw<string[]>([]);
	let revenue = $state(false);
	let funnels = $state.raw<Funnel[]>([]);
	let titles = $state.raw<Record<string, string>>({});
	let propNames = $state.raw<string[]>([]);
	let notes = $state.raw<Note[]>([]);
	let noting = $state<string | null>(null);

	function navigate(change: (params: URLSearchParams) => void) {
		const params = new URLSearchParams(page.url.search);
		change(params);
		// the current path is already resolved, under /[site] or /share/[slug]
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		goto(`${page.url.pathname}?${params}`, { reset: false });
	}

	const assign = (to: Record<string, string | null>) => (params: URLSearchParams) => {
		for (const [key, value] of Object.entries(to))
			if (value === null) params.delete(key);
			else params.set(key, value);
	};

	const go = (to: Record<string, string | null>) => navigate(assign(to));

	function ahead(to: Record<string, string | null>) {
		const params = new URLSearchParams(page.url.search);
		assign(to)(params);
		warm(domain, timezone, params, layout.table);
	}

	const applyFilters = (raw: string[]) =>
		navigate((params) => {
			params.delete("f");
			for (const item of raw) params.append("f", item);
		});

	function setFilter(filter: Filter) {
		editing = undefined;
		applyFilters([
			...filters.filter((item) => item.dim !== filter.dim).map(encodeFilter),
			encodeFilter(filter)
		]);
	}

	function removeFilter(dim: string) {
		editing = undefined;
		applyFilters(filters.filter((item) => item.dim !== dim).map(encodeFilter));
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
			propKeys: propNames,
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
		void checks;
		failed = false;
		cached(domain, { metrics: ["events"], date_range: "all" })
			.then((result) => {
				if (!stale) empty = !result.results[0]?.metrics[0];
			})
			.catch(() => {
				if (!stale) failed = true;
			});
		return () => (stale = true);
	});

	$effect(() => {
		if (!editable || combined) return;
		newEvents(domain)
			.then((events) => (unlisted = events))
			.catch(() => (unlisted = []));
	});

	function loadGoals() {
		get<{ goals: { name: string; currency: string }[] }>(`/api/app/sites/${domain}/goals`)
			.then((result) => {
				goals = result.goals.map((goal) => goal.name);
				revenue = result.goals.some((goal) => goal.currency);
			})
			.catch(() => (goals = []));
	}

	$effect(() => {
		if (combined) return;
		get<Record<string, string>>(`/api/app/sites/${domain}/titles`)
			.then((result) => (titles = result))
			.catch(() => (titles = {}));
		get<{ funnels: Funnel[] }>(`/api/app/sites/${domain}/funnels`)
			.then((result) => (funnels = result.funnels))
			.catch(() => (funnels = []));
		get<{ notes: Note[] }>(`/api/app/sites/${domain}/notes`)
			.then((result) => (notes = result.notes))
			.catch(() => (notes = []));
		untrack(loadGoals);
	});

	$effect(() => {
		void version;
		if (!dates || empty) return;
		propKeys(domain, dates.current).then((keys) => (propNames = keys));
	});

	async function done() {
		layout.arranging = false;
		await tick();
		document.querySelector<HTMLElement>('[aria-label="Dashboard options"]')?.focus();
	}
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && layout.arranging && done()} />

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
			propKeys={propNames}
			initial={editing ?? null}
			apply={setFilter}
			remove={removeFilter}
		/>
	{/if}
</Dialog>

<Dialog
	title="notes"
	bind:open={
		() => noting !== null,
		(open) => {
			if (!open) noting = null;
		}
	}
>
	{#if noting !== null}
		<Notes {domain} bind:notes day={noting} {editable} />
	{/if}
</Dialog>

<Dialog title="new events" bind:open={reviewing}>
	<NewEvents
		{domain}
		bind:events={unlisted}
		changed={() => {
			loadGoals();
			if (unlisted.length === 0) reviewing = false;
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
				title
				footer={editable
					? {
							label: "Site settings",
							href: resolve("/[site]/settings/[[section]]", { site: domain }),
							icon: sliders
						}
					: undefined}
				warm={(key) => {
					const site = findSite(key);
					if (team && key === "all") warm(`team:${team.id}`, teamTimezone(team));
					else if (site) warm(key, site.timezone);
				}}
				pick={(key) =>
					goto(key === "all" ? resolve("/all") : resolve("/[site]", { site: key }))}
			/>
		{/if}
		{#if empty === false}<Live site={domain} />{/if}
		{#if empty === false && uptime.value?.monitor}
			<a
				href={resolve("/uptime/[id]", { id: String(uptime.value.monitor.id) })}
				class="flex h-control animate-[pop_100ms_ease-out] items-center rounded-px px-1 hover:bg-raised motion-reduce:animate-none"
				title="uptime"
			>
				<State state={uptime.value.monitor.state} since={uptime.value.monitor.since} />
			</a>
		{/if}
		<div class="flex items-center gap-2 max-sm:w-full sm:ml-auto">
			{#if layout.arranging}
				<Button
					variant="quiet"
					class="ml-auto"
					onclick={done}
					{@attach (button: HTMLElement) => button.focus()}>Done</Button
				>
			{:else if empty === false}
				<Button variant="plain" onclick={() => (editing = null)}
					><Icon svg={funnel} size={12} />Filter</Button
				>
				{#if view}
					<Period {view} {go} warm={ahead} />
					{#if !readonly}
						<div class="max-sm:ml-auto">
							<Menu
								label="Dashboard options"
								icon={more}
								plain
								iconOnly
								align="right"
								items={[
									{
										label: "Edit layout",
										onclick: () => (layout.arranging = true)
									},
									...(combined
										? []
										: [
												{
													label: "Notes",
													onclick: () => (noting = view.today)
												}
											]),
									...(hasImports
										? [
												{
													label: "Include imported data",
													current: imports,
													onclick: () =>
														go({ imports: imports ? "0" : null })
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
						</div>
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
		apply={applyFilters}
	/>

	{#if editable && !combined && empty === false && unlisted.length}
		<Banner
			text={unlisted.length === 1
				? `Your site sends ${unlisted[0].name} events, but there's no goal for them yet.`
				: `Your site sends ${unlisted.length} kinds of events that have no goal yet.`}
		>
			<Button variant="quiet" onclick={() => (reviewing = true)}>Review</Button>
		</Banner>
	{/if}

	{#if empty && !combined}
		{#key domain}
			<Waiting
				{domain}
				landed={() => {
					empty = false;
					version++;
				}}
			/>
		{/key}
	{:else if failed}
		<Empty retry={() => checks++} />
	{:else if ctx && layout.ready}
		<Overview
			{ctx}
			metric={realtime ? "pageviews" : metric}
			{realtime}
			incidents={uptime.value?.incidents}
			notes={combined ? [] : notes}
			note={editable && !combined ? (day) => (noting = day) : undefined}
			table={layout.table}
			found={(has) => (hasImports = has)}
			pick={(next) => navigate((params) => params.set("metric", next))}
			setTable={layout.setTable}
			setGrain={(grain) => go({ interval: grain })}
		/>
		<Grid {ctx} {layout} />
		{#if readonly}
			<Credit label="Dashboard" />
		{/if}
	{:else}
		<Loading bare />
	{/if}
</div>
