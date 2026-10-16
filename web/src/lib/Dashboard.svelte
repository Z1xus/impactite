<script lang="ts">
	import { untrack } from "svelte";
	import { flip } from "svelte/animate";
	import { clock } from "#lib/display.svelte.ts";
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import Chart from "#lib/dither/Chart.svelte";
	import WorldMap from "#lib/dither/Map.svelte";
	import { api } from "#lib/api.ts";
	import { save } from "#lib/download.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import { ICONIC, LETTERED, icon } from "#lib/icons/resolve.ts";
	import bookmark from "pixelarticons/svg/bookmark.svg?raw";
	import more from "pixelarticons/svg/more-vertical.svg?raw";
	import funnel from "pixelarticons/svg/filter.svg?raw";
	import sliders from "pixelarticons/svg/sliders.svg?raw";
	import { currentTeam, session } from "#lib/session.svelte.ts";
	import {
		METRICS,
		OPS,
		decodeFilter,
		dimLabel,
		display,
		encodeFilter,
		format,
		query,
		toQueryFilters,
		type Filter
	} from "#lib/stats.ts";
	import Period from "#lib/Period.svelte";
	import { current, interval, intervals, parse, previous } from "#lib/period.ts";
	import Details from "#lib/Details.svelte";
	import FilterDialog from "#lib/FilterDialog.svelte";
	import NewEvents from "#lib/NewEvents.svelte";
	import Waiting from "#lib/Waiting.svelte";
	import Button from "#lib/ui/Button.svelte";
	import AddPanel from "#lib/ui/AddPanel.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Menu from "#lib/ui/Menu.svelte";
	import Table from "#lib/ui/Table.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Checks from "#lib/ui/Checks.svelte";
	import Chip from "#lib/ui/Chip.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Glyph from "#lib/ui/Glyph.svelte";
	import Title from "#lib/ui/Title.svelte";
	import Kpis from "#lib/ui/Kpis.svelte";
	import Live from "#lib/ui/Live.svelte";
	import Panel from "#lib/ui/Panel.svelte";
	import Rows from "#lib/ui/Rows.svelte";
	import Select from "#lib/ui/Select.svelte";

	type Ranked = Parameters<typeof Rows>[1]["rows"];

	const PAGES = new Set(["event:page", "visit:entry_page", "visit:exit_page"]);

	let {
		domain,
		timezone,
		readonly = false
	}: { domain: string; timezone: string; readonly?: boolean } = $props();

	const member = $derived(
		session.me?.teams.flatMap((team) => team.sites).find((site) => site.domain === domain)
	);
	const editable = $derived(
		!readonly && ["owner", "admin", "editor"].includes(member?.role ?? "")
	);
	let newEvents = $state<{ name: string; count: number; revenue: boolean }[]>([]);
	let reviewing = $state(false);
	$effect(() => {
		if (!editable || domain.startsWith("team:")) return;
		api<{ events: typeof newEvents }>(`/api/app/sites/${domain}/goals/suggestions`)
			.then((r) => (newEvents = r.events))
			.catch(() => (newEvents = []));
	});
	// `team:<id>` shows every site of the team added up
	const combined = $derived(domain.startsWith("team:"));
	const team = $derived(currentTeam());
	const sites = $derived([
		...((team?.sites.length ?? 0) > 1 ? [{ key: "all", label: "All sites" }] : []),
		...(session.me?.teams.flatMap((team) => team.sites) ?? []).map((site) => ({
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
	const withImports = $derived(page.url.searchParams.get("imports") !== "0");
	const ask = (body: Record<string, unknown>) => query(domain, body, withImports);

	const realtime = $derived(view?.period === "realtime");
	const chartMetric = $derived(realtime ? "pageviews" : metric);

	$effect(() => {
		if (!realtime) return;
		const timer = setInterval(() => version++, 30_000);
		return () => clearInterval(timer);
	});

	let hasImports = $state(false);
	let empty = $state<boolean | null>(null);
	let version = $state(0);

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

	let totals = $state<number[] | null>(null);
	let previousTotals = $state<number[] | null>(null);
	let series = $state<number[]>([]);
	let lines = $state<{ key: string; label: string; values: number[] }[]>([]);
	let lineSort = $state<[string, "asc" | "desc"]>(["", "asc"]);
	const sortedLines = $derived.by(() => {
		const i = METRICS.findIndex((item) => item.key === lineSort[0]);
		if (i < 0) return lines;
		const sign = lineSort[1] === "desc" ? -1 : 1;
		return [...lines].sort((a, b) => sign * (a.values[i] - b.values[i]));
	});
	// realtime minutes only count pageviews
	const tableMetrics = $derived(
		realtime ? METRICS.filter((item) => item.key === "pageviews") : METRICS
	);

	type Layout = { order: string[]; hidden: string[]; table: boolean };
	let layout = $state<Layout>({ order: [], hidden: [], table: false });
	let arranging = $state(false);
	let dragging = $state<string | null>(null);
	let dropTarget = $state<string | null>(null);

	$effect(() => {
		if (readonly) return;
		api<Partial<Layout> | null>(`/api/app/prefs/layout:${domain}`).then((saved) => {
			const list = (value: unknown) =>
				Array.isArray(value) ? value.filter((item) => typeof item === "string") : [];
			layout = {
				order: list(saved?.order),
				hidden: list(saved?.hidden),
				table: saved?.table === true
			};
		});
	});

	function saveLayout(next: Layout) {
		layout = next;
		if (!readonly) api(`/api/app/prefs/layout:${domain}`, next, "PUT");
	}
	let previousSeries = $state<number[]>([]);
	let labels = $state<string[]>([]);

	const base = [
		{
			id: "sources",
			tabs: [
				{ key: "visit:source", label: "sources" },
				{ key: "visit:channel", label: "channels" },
				{ key: "utm", label: "utm" }
			]
		},
		{
			id: "pages",
			tabs: [
				{ key: "event:page", label: "pages" },
				{ key: "visit:entry_page", label: "entry" },
				{ key: "visit:exit_page", label: "exit" },
				{ key: "event:hostname", label: "hostnames" }
			]
		},
		{
			id: "locations",
			tabs: [
				{ key: "map", label: "map" },
				{ key: "visit:country", label: "countries" },
				{ key: "visit:region", label: "regions" },
				{ key: "visit:city", label: "cities" }
			]
		},
		{
			id: "devices",
			tabs: [
				{ key: "visit:browser", label: "browsers" },
				{ key: "visit:os", label: "systems" },
				{ key: "visit:device", label: "devices" },
				{ key: "visit:language", label: "languages" }
			]
		}
	];
	let goals = $state<string[]>([]);
	let titles = $state<Record<string, string>>({});
	// search console terms replace referrers under the google source when connected
	let terms = $state(false);
	let funnels = $state<{ id: number; name: string }[]>([]);
	let funnelId = $state(0);
	let revenue = $state(false);
	let propKeys = $state<string[]>([]);
	let propKey = $state("");
	const panels = $derived([
		...base,
		{
			id: "goals",
			tabs: [
				...(goals.length ? [{ key: "event:goal", label: "goals" }] : []),
				...(propKeys.length ? [{ key: "props", label: "props" }] : []),
				...(funnels.length ? [{ key: "funnels", label: "funnels" }] : [])
			]
		}
	]);
	let tabs = $state<Record<string, string>>({
		...Object.fromEntries(base.map((panel) => [panel.id, panel.tabs[0].key])),
		goals: "event:goal"
	});

	$effect(() => {
		const goal = panels.at(-1)?.tabs ?? [];
		if (goal.length && !goal.some((item) => item.key === tabs.goals)) tabs.goals = goal[0].key;
	});

	function loadGoals() {
		api<{ goals: { name: string; currency: string }[] }>(`/api/app/sites/${domain}/goals`)
			.then((result) => {
				goals = result.goals.map((goal) => goal.name);
				revenue = result.goals.some((goal) => goal.currency);
			})
			.catch(() => (goals = []));
	}

	$effect(() => {
		// the all sites view has no goals, funnels, titles or segments of its own
		if (combined) return;
		api<Record<string, string>>(`/api/app/sites/${domain}/titles`)
			.then((result) => (titles = result))
			.catch(() => (titles = {}));
		untrack(loadGoals);
		api<{ funnels: { id: number; name: string }[] }>(`/api/app/sites/${domain}/funnels`)
			.then((result) => {
				funnels = result.funnels;
				if (!funnels.some((funnel) => funnel.id === funnelId))
					funnelId = funnels[0]?.id ?? 0;
			})
			.catch(() => (funnels = []));
	});

	$effect(() => {
		void version;
		if (!dates || empty !== false) return;
		const range = dates.current === "30m" ? "day" : dates.current;
		api<string[]>("/api/app/props", { site_id: domain, date_range: range }).then((keys) => {
			propKeys = keys;
			if (!keys.includes(propKey)) propKey = keys[0] ?? "";
		});
	});
	let ranked = $state<Record<string, Ranked>>({});
	let loading = $state<Record<string, boolean>>({});

	const downloadReport = () =>
		attempt(async () => {
			if (!dates) return;
			const site = domain;
			const range = dates.current;
			const grain = dates.grain;
			const result = await query(
				site,
				{
					metrics: METRICS.map((item) => item.key),
					date_range: range,
					dimensions: [grain],
					filters: toQueryFilters(filters)
				},
				withImports
			);
			const rows = [
				[
					grain.slice(5),
					"visitors",
					"visits",
					"pageviews",
					"bounce_rate_percent",
					"visit_duration_seconds",
					"views_per_visit"
				],
				...result.results.map((row) => [...row.dimensions, ...row.metrics])
			];
			const csv =
				rows
					.map((row) =>
						row.map((value) => `"${String(value).replaceAll('"', '""')}"`).join(",")
					)
					.join("\r\n") + "\r\n";
			const period = Array.isArray(range) ? range.join("_") : range;
			save(`${site}_${period}.csv`, new Blob([csv], { type: "text/csv;charset=utf-8" }));
		});

	function go(change: Record<string, string | null>) {
		navigate((params) => {
			for (const [key, value] of Object.entries(change))
				if (value === null) params.delete(key);
				else params.set(key, value);
		});
	}

	function navigate(change: (params: URLSearchParams) => void) {
		const params = new URLSearchParams(page.url.search);
		change(params);
		// the current path is already resolved, under /[site] or /share/[slug]
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		goto(`${page.url.pathname}?${params}`, { reset: false });
	}

	let editing = $state<Filter | null | undefined>(undefined);
	let details = $state<{ dim: string; title: string } | null>(null);
	const converting = $derived(filters.some((filter) => filter.dim === "event:goal"));
	const UTM = [
		{ key: "visit:utm_campaign", label: "campaign" },
		{ key: "visit:utm_source", label: "source" },
		{ key: "visit:utm_medium", label: "medium" },
		{ key: "visit:utm_content", label: "content" },
		{ key: "visit:utm_term", label: "term" }
	];
	// one value filtered on these shows the level below, like plausible
	const DRILL: Record<string, { dim: string; label: string }> = {
		"visit:source": { dim: "visit:referrer", label: "referrers" },
		"visit:browser": { dim: "visit:browser_version", label: "versions" },
		"visit:os": { dim: "visit:os_version", label: "versions" }
	};
	let utmKey = $state("visit:utm_campaign");
	const drilled = (tab: string) => {
		const filter = filters.find((item) => item.dim === tab);
		return filter?.op === "is" && filter.values.length === 1 ? DRILL[tab] : undefined;
	};
	const arranged = $derived.by(() => {
		const all = panels.filter((panel) => panel.tabs.length);
		const rank = (id: string) => {
			const i = layout.order.indexOf(id);
			return i < 0 ? 1000 + all.findIndex((panel) => panel.id === id) : i;
		};
		return [...all].sort((a, b) => rank(a.id) - rank(b.id));
	});
	const shown = $derived(arranged.filter((panel) => !layout.hidden.includes(panel.id)));

	function dragOver(event: DragEvent) {
		if (!dragging) return;
		event.preventDefault();
		if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
		const grid = event.currentTarget as HTMLElement;
		let closest: string | null = null;
		let distance = Infinity;
		for (const panel of grid.querySelectorAll<HTMLElement>(":scope > [data-panel]")) {
			const box = panel.getBoundingClientRect();
			const dx = Math.max(box.left - event.clientX, 0, event.clientX - box.right);
			const dy = Math.max(box.top - event.clientY, 0, event.clientY - box.bottom);
			const next = dx * dx + dy * dy;
			if (next < distance) {
				distance = next;
				closest = panel.dataset.panel ?? null;
			}
		}
		dropTarget = closest === dragging ? null : closest;
	}

	function drop(event: DragEvent) {
		if (!dragging) return;
		dragOver(event);
		if (!dropTarget) return dragEnd();
		const ids = arranged.map((panel) => panel.id).filter((item) => item !== dragging);
		ids.splice(
			ids.indexOf(dropTarget) +
				(arranged.findIndex((panel) => panel.id === dragging) <
				arranged.findIndex((panel) => panel.id === dropTarget)
					? 1
					: 0),
			0,
			dragging
		);
		saveLayout({ ...layout, order: ids });
		dragEnd();
	}

	function dragStart(event: DragEvent, id: string) {
		dragging = id;
		dropTarget = null;
		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = "move";
			event.dataTransfer.setData("text/plain", id);
		}
	}

	function dragEnd() {
		dragging = null;
		dropTarget = null;
	}

	const hidePanel = (id: string) => saveLayout({ ...layout, hidden: [...layout.hidden, id] });

	const dimOf = (id: string) => {
		const tab = tabs[id];
		if (tab === "props") return `event:props:${propKey}`;
		if (tab === "utm") return utmKey;
		if (tab === "map") return "visit:country";
		return drilled(tab)?.dim ?? tab;
	};

	type Segment = { id: number; name: string; filters: string[]; shared: boolean; mine: boolean };
	let segments = $state<Segment[]>([]);
	let saving = $state(false);
	let segmentName = $state("");
	let segmentShared = $state<string[]>([]);
	const active = $derived(
		segments.find(
			(segment) => segment.filters.join("\n") === page.url.searchParams.getAll("f").join("\n")
		)
	);

	$effect(() => {
		if (readonly || combined) return;
		api<{ segments: Segment[] }>(`/api/app/sites/${domain}/segments`)
			.then((result) => (segments = result.segments))
			.catch(() => (segments = []));
	});

	function pickSegment(key: string) {
		if (readonly || combined) return;
		if (key === "save") {
			saving = true;
			return;
		}
		const segment = segments.find((item) => String(item.id) === key);
		navigate((params) => {
			params.delete("f");
			for (const raw of segment?.filters ?? []) params.append("f", raw);
		});
	}

	async function saveSegment() {
		if (readonly || combined) return;
		segments = (
			await api<{ segments: Segment[] }>(`/api/app/sites/${domain}/segments`, {
				name: segmentName,
				filters: page.url.searchParams.getAll("f"),
				shared: segmentShared.length > 0
			})
		).segments;
		saving = false;
		segmentName = "";
	}

	function setFilter(filter: Filter) {
		editing = undefined;
		navigate((params) => {
			const rest = params.getAll("f").filter((raw) => decodeFilter(raw)?.dim !== filter.dim);
			params.delete("f");
			for (const raw of [...rest, encodeFilter(filter)]) params.append("f", raw);
		});
	}

	function addFilter(dim: string, value: string) {
		setFilter({ op: "is", dim, values: [value] });
	}

	function removeFilter(dim: string) {
		editing = undefined;
		navigate((params) => {
			const rest = params.getAll("f").filter((raw) => decodeFilter(raw)?.dim !== dim);
			params.delete("f");
			for (const raw of rest) params.append("f", raw);
		});
	}

	function label(raw: string, grain: string) {
		const date = new Date(raw.replace(" ", "T") + (raw.length === 10 ? "T00:00:00" : ""));
		if (grain === "time:hour" || grain === "time:minute") return clock(raw.slice(11, 16));
		const options: Intl.DateTimeFormatOptions =
			grain === "time:month"
				? { month: "short", year: "2-digit" }
				: { month: "short", day: "numeric" };
		return date.toLocaleDateString("en-US", options);
	}

	$effect(() => {
		void version;
		if (!dates || empty !== false) return;
		const where = toQueryFilters(filters);
		const keys = METRICS.map((item) => item.key);
		let stale = false;
		const { previous: before, grain } = dates;
		Promise.all([
			ask({ metrics: keys, date_range: dates.current, filters: where }),
			before && ask({ metrics: keys, date_range: before, filters: where }),
			ask({
				metrics: layout.table ? keys : [chartMetric],
				date_range: dates.current,
				dimensions: [grain],
				filters: where
			}),
			before &&
				dates.overlay &&
				ask({
					metrics: [chartMetric],
					date_range: before,
					dimensions: [grain],
					filters: where
				})
		]).then(([now, before, line, previousLine]) => {
			if (stale) return;
			hasImports =
				!withImports || (now.meta?.imports_skip_reason ?? "") !== "no_imported_data";
			totals = now.results[0]?.metrics ?? null;
			previousTotals = before ? (before.results[0]?.metrics ?? null) : null;
			const at = layout.table ? keys.indexOf(chartMetric) : 0;
			series = line.results.map((row) => row.metrics[Math.max(at, 0)]);
			lines = line.results.map((row) => ({
				key: row.dimensions[0],
				label: label(row.dimensions[0], grain),
				values: row.metrics
			}));
			previousSeries = previousLine ? previousLine.results.map((row) => row.metrics[0]) : [];
			labels = line.results.map((row) => label(row.dimensions[0], grain));
		});
		return () => (stale = true);
	});

	$effect(() => {
		void version;
		if (!dates || empty !== false) return;
		const where = toQueryFilters(filters);
		const cleanups: (() => void)[] = [];
		terms = false;
		for (const panel of panels) {
			if (!panel.tabs.length) continue;
			const dim = dimOf(panel.id);
			let stale = false;
			loading[panel.id] = true;
			if (tabs[panel.id] === "funnels") {
				api<{ steps: { name: string; visitors: number }[] }>("/api/app/funnel", {
					site_id: domain,
					funnel: funnelId,
					date_range: dates.current === "30m" ? "day" : dates.current,
					filters: where
				}).then((result) => {
					if (stale) return;
					const first = result.steps[0]?.visitors || 1;
					ranked[panel.id] = result.steps.map((step, i) => ({
						key: String(i),
						label: `${i + 1}. ${step.name}`,
						value: step.visitors,
						extra: `${Math.round((step.visitors / first) * 1000) / 10}%`
					}));
					loading[panel.id] = false;
				});
				cleanups.push(() => (stale = true));
				continue;
			}
			const regular = () =>
				ask({
					metrics:
						dim === "event:goal" || converting
							? ["visitors", "conversion_rate"]
							: ["visitors"],
					date_range: dates.current,
					dimensions: [dim],
					filters: where,
					pagination: { limit: tabs[panel.id] === "map" ? 300 : 9 }
				}).then((result) => {
					if (stale) return;
					ranked[panel.id] = result.results.map((row) => {
						const value = row.dimensions[0];
						const label = display(dim, value);
						return {
							key: value,
							label,
							value: row.metrics[0],
							...((dim === "event:goal" || converting) && {
								extra: `${row.metrics[1]}%`
							}),
							...(!combined &&
								PAGES.has(dim) &&
								value.startsWith("/") && { href: `https://${domain}${value}` }),
							...(ICONIC.has(dim) && {
								icon: icon(dim, value),
								fallback: LETTERED.has(dim) ? label : undefined
							})
						};
					});
					loading[panel.id] = false;
				});
			const google = filters.find((item) => item.dim === "visit:source");
			if (
				!combined &&
				tabs[panel.id] === "visit:source" &&
				google?.op === "is" &&
				google.values.length === 1 &&
				google.values[0] === "Google"
			) {
				const page = filters.find(
					(item) =>
						item.dim === "event:page" && item.op === "is" && item.values.length === 1
				)?.values[0];
				api<{ keywords: { name: string; visitors: number; position: number }[] }>(
					"/api/app/keywords",
					{
						site_id: domain,
						date_range: dates.current === "30m" ? "day" : dates.current,
						page
					}
				)
					.then((result) => {
						if (stale) return;
						terms = true;
						ranked[panel.id] = result.keywords.map((keyword) => ({
							key: keyword.name,
							label: keyword.name,
							value: keyword.visitors,
							extra: `#${Math.round(keyword.position)}`
						}));
						loading[panel.id] = false;
					})
					.catch(regular);
			} else regular();
			cleanups.push(() => (stale = true));
		}
		return () => cleanups.forEach((cleanup) => cleanup());
	});
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && (arranging = false)} />

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

<Dialog title="save segment" bind:open={saving}>
	<Form action="save segment" submit={saveSegment}>
		<Field label="segment name" bind:value={segmentName} required />
		{#if editable}
			<Checks
				bind:value={segmentShared}
				options={[{ key: "shared", label: "share with everyone on this site" }]}
			/>
		{/if}
	</Form>
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
			imports={withImports}
			{converting}
			{revenue}
			iconic={ICONIC.has(details.dim)}
			pick={(value) => {
				const dim = details?.dim ?? "";
				details = null;
				addFilter(dim, value);
			}}
		/>
	{/if}
</Dialog>

{#snippet funnelPicker()}
	<Select
		align="right"
		value={funnelId}
		options={funnels.map((funnel) => ({ key: funnel.id, label: funnel.name }))}
		data
		pick={(id) => (funnelId = id)}
	/>
{/snippet}

{#snippet utmPicker()}
	<Select align="right" value={utmKey} options={UTM} pick={(key) => (utmKey = key)} />
{/snippet}

{#snippet picker()}
	<Select
		align="right"
		value={propKey}
		options={propKeys.map((key) => ({ key, label: key }))}
		data
		pick={(key) => (propKey = key)}
	/>
{/snippet}

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
		<div class="ml-auto flex items-center gap-2">
			{#if empty === false}
				<Button variant="text" onclick={() => (editing = null)}
					><Icon svg={funnel} size={14} />Filter</Button
				>
			{/if}
			{#if view && empty === false}
				<Period {view} {go} />
				{#if !readonly}
					{#if arranging}<Button variant="quiet" onclick={() => (arranging = false)}
							>Done</Button
						>{/if}
					<Menu
						label="Dashboard options"
						icon={more}
						plain
						iconOnly
						align="right"
						items={[
							{ label: "Edit layout", onclick: () => (arranging = true) },
							...(hasImports
								? [
										{
											label: "Include imported data",
											current: withImports,
											onclick: () => go({ imports: withImports ? "0" : null })
										}
									]
								: []),
							"divider",
							{ label: "Download CSV", onclick: downloadReport }
						]}
					/>
				{/if}
			{/if}
		</div>
	</div>

	{#if filters.length || (!readonly && !combined && empty === false && segments.length)}
		<div class="flex flex-wrap items-center gap-2">
			{#each filters as filter (encodeFilter(filter))}
				<Chip
					label={filter.op === "is"
						? dimLabel(filter.dim)
						: `${dimLabel(filter.dim)} ${OPS.find((op) => op.key === filter.op)?.label}`}
					value={filter.values.map((value) => display(filter.dim, value)).join(" or ")}
					icon={ICONIC.has(filter.dim) && filter.values.length === 1
						? icon(filter.dim, filter.values[0])
						: undefined}
					edit={() => (editing = filter)}
					remove={() => removeFilter(filter.dim)}
				/>
			{/each}
			{#if !readonly && !combined && empty === false && (segments.length || filters.length)}
				<Select
					value={active ? String(active.id) : ""}
					text={active?.name ?? "Segments"}
					plain
					icon={bookmark}
					data
					options={[
						...segments.map((segment) => ({
							key: String(segment.id),
							label: segment.name
						})),
						...(filters.length && !active
							? [{ key: "save", label: "Save as segment" }]
							: [])
					]}
					pick={pickSegment}
				/>
			{/if}
		</div>
	{/if}

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
	{:else if empty === false}
		<section class="rounded-px border border-line bg-panel">
			<Kpis
				{totals}
				previous={previousTotals}
				active={METRICS.findIndex((item) => item.key === chartMetric)}
				pick={(i) => navigate((params) => params.set("metric", METRICS[i].key))}
			/>
			<div class="px-4 pt-3 pb-5">
				<div class="flex items-center justify-end gap-2 {layout.table ? '' : 'mb-2'}">
					{#if dates && dates.choices.length > 1}
						<Select
							align="right"
							plain
							value={dates.interval}
							options={dates.choices.map((key) => ({ key, label: key }))}
							pick={(key) => go({ interval: key })}
						/>
					{/if}
					<Toggle
						plain
						bind:value={
							() => (layout.table ? "table" : "chart"),
							(view: "chart" | "table") =>
								saveLayout({ ...layout, table: view === "table" })
						}
						options={[
							{ key: "chart", label: "Chart" },
							{ key: "table", label: "Table" }
						]}
					/>
				</div>
				{#if layout.table}
					<div class="max-h-96 [scrollbar-gutter:stable] overflow-y-auto">
						<Table
							columns={tableMetrics.map((item) => ({
								key: item.key,
								label: item.label,
								format: (value: number | null) => format(item.key, value ?? 0)
							}))}
							rows={sortedLines.map((line) => ({
								...line,
								values: tableMetrics.map(
									(item) => line.values[METRICS.indexOf(item)]
								)
							}))}
							bind:sort={lineSort}
						/>
					</div>
				{:else}
					<Chart
						values={series}
						previous={previousSeries}
						{labels}
						keys={lines.map((line) => line.key)}
						format={(value) => format(chartMetric, value)}
					/>
				{/if}
			</div>
		</section>

		<div
			class="grid grid-cols-1 gap-6 md:grid-cols-2"
			role="list"
			ondragover={dragOver}
			ondrop={drop}
			ondragleave={(event) => {
				if (
					!(event.currentTarget as HTMLElement).contains(
						event.relatedTarget as Node | null
					)
				)
					dropTarget = null;
			}}
		>
			{#each arranging ? arranged : shown as panel (panel.id)}
				<div
					role="listitem"
					data-panel={panel.id}
					animate:flip={{ duration: 200 }}
					class="min-w-0 transition-opacity duration-150 {panel.id === 'goals' &&
					!layout.hidden.includes(panel.id)
						? 'md:col-span-2'
						: ''} {dragging === panel.id ? 'opacity-40' : ''} {arranging
						? 'select-none'
						: ''} {dropTarget === panel.id
						? 'outline -outline-offset-1 outline-cyan'
						: ''}"
					draggable={arranging && !layout.hidden.includes(panel.id)}
					ondragstart={(event) => dragStart(event, panel.id)}
					ondragend={dragEnd}
				>
					{#if layout.hidden.includes(panel.id)}
						<AddPanel
							label={panel.tabs[0].label}
							onclick={() =>
								saveLayout({
									...layout,
									hidden: layout.hidden.filter((id) => id !== panel.id)
								})}
						/>
					{:else}
						<Panel
							{arranging}
							fit={panel.id === "goals"}
							hide={() => hidePanel(panel.id)}
							tabs={panel.tabs.map((tab) => ({
								...tab,
								label:
									terms && tab.key === "visit:source" && drilled(tab.key)
										? "search terms"
										: (drilled(tab.key)?.label ?? tab.label)
							}))}
							bind:tab={tabs[panel.id]}
							expand={tabs[panel.id] === "funnels"
								? undefined
								: () =>
										(details = {
											dim: dimOf(panel.id),
											title:
												panel.tabs.find((tab) => tab.key === tabs[panel.id])
													?.label ?? ""
										})}
							actions={tabs[panel.id] === "props"
								? picker
								: tabs[panel.id] === "utm"
									? utmPicker
									: tabs[panel.id] === "funnels"
										? funnelPicker
										: undefined}
						>
							{#if tabs[panel.id] === "map"}
								<div class="px-2 py-1">
									<WorldMap
										values={Object.fromEntries(
											(ranked[panel.id] ?? []).map((row) => [
												row.key,
												row.value
											])
										)}
										label={(code) => display("visit:country", code)}
										format={(value) => format("visitors", value)}
										pick={(code) => addFilter("visit:country", code)}
									/>
								</div>
							{:else}
								<Rows
									rows={PAGES.has(dimOf(panel.id))
										? (ranked[panel.id]?.map((row) => ({
												...row,
												note: titles[row.key]
											})) ?? null)
										: (ranked[panel.id] ?? null)}
									loading={loading[panel.id]}
									format={(value) => format("visitors", value)}
									columns={tabs[panel.id] === "funnels"
										? ["rate", "visitors"]
										: terms && tabs[panel.id] === "visit:source"
											? ["rank", "visitors"]
											: dimOf(panel.id) === "event:goal" || converting
												? ["CR", "visitors"]
												: ["visitors"]}
									pick={tabs[panel.id] === "funnels" ||
									(terms &&
										tabs[panel.id] === "visit:source" &&
										drilled("visit:source"))
										? undefined
										: (key) => addFilter(dimOf(panel.id), key)}
								/>
							{/if}
						</Panel>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>
