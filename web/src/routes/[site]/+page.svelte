<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import Chart from "#lib/dither/Chart.svelte";
	import { api } from "#lib/api.ts";
	import { icon } from "#lib/icons/resolve.ts";
	import bookmark from "pixelarticons/svg/bookmark.svg?raw";
	import funnel from "pixelarticons/svg/filter.svg?raw";
	import sliders from "pixelarticons/svg/sliders.svg?raw";
	import { session } from "#lib/session.svelte.ts";
	import {
		METRICS,
		OPS,
		decodeFilter,
		dimLabel,
		delta,
		display,
		encodeFilter,
		format,
		query,
		toQueryFilters,
		type Filter,
		type Metric
	} from "#lib/stats.ts";
	import Period from "#lib/Period.svelte";
	import { current, interval, intervals, parse, previous } from "#lib/period.ts";
	import Details from "#lib/Details.svelte";
	import FilterDialog from "#lib/FilterDialog.svelte";
	import Snippet from "#lib/Snippet.svelte";
	import Button from "#lib/ui/Button.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Checks from "#lib/ui/Checks.svelte";
	import Chip from "#lib/ui/Chip.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Kpi from "#lib/ui/Kpi.svelte";
	import Live from "#lib/ui/Live.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Panel from "#lib/ui/Panel.svelte";
	import Rows from "#lib/ui/Rows.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Shell from "#lib/ui/Shell.svelte";

	type Ranked = Parameters<typeof Rows>[1]["rows"];

	const ICONIC = new Set([
		"visit:source",
		"visit:referrer",
		"visit:channel",
		"event:hostname",
		"visit:country",
		"visit:region",
		"visit:browser",
		"visit:os",
		"visit:device"
	]);
	const LETTERED = new Set([
		"visit:source",
		"visit:referrer",
		"event:hostname",
		"visit:browser",
		"visit:os"
	]);

	const PAGES = new Set(["event:page", "visit:entry_page", "visit:exit_page"]);

	const domain = $derived(page.params.site ?? "");
	const site = $derived(
		session.me?.teams.flatMap((team) => team.sites).find((site) => site.domain === domain)
	);
	const editable = $derived(["owner", "admin", "editor"].includes(site?.role ?? ""));
	const sites = $derived(
		(session.me?.teams.flatMap((team) => team.sites) ?? []).map((site) => ({
			key: site.domain,
			label: site.domain,
			glyph: icon("event:hostname", site.domain)
		}))
	);
	const metric = $derived((page.url.searchParams.get("metric") ?? "visitors") as Metric);
	const filters = $derived(
		page.url.searchParams
			.getAll("f")
			.map(decodeFilter)
			.filter((filter): filter is Filter => filter !== null)
	);
	const view = $derived(site ? parse(page.url.searchParams, site.timezone) : null);
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
	let quiet = $state(false);
	let empty = $state(false);
	let version = $state(0);

	$effect(() => {
		if (!quiet) return;
		const site = domain;
		let timer = 0;
		const check = () =>
			query(site, { metrics: ["events"], date_range: "all" }).then((result) => {
				if (!result.results[0]?.metrics[0]) {
					empty = true;
					timer = window.setTimeout(check, 3000);
				} else if (empty) {
					empty = false;
					quiet = false;
					version++;
				}
			});
		check();
		return () => clearTimeout(timer);
	});

	let totals = $state<number[] | null>(null);
	let previousTotals = $state<number[] | null>(null);
	let series = $state<number[]>([]);
	let previousSeries = $state<number[]>([]);
	let labels = $state<string[]>([]);

	const base = [
		{
			id: "sources",
			tabs: [
				{ key: "visit:source", label: "sources" },
				{ key: "visit:channel", label: "channels" },
				{ key: "visit:utm_campaign", label: "campaigns" }
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
				{ key: "visit:device", label: "devices" }
			]
		}
	];
	let goals = $state<string[]>([]);
	let propKeys = $state<string[]>([]);
	let propKey = $state("");
	const panels = $derived([
		...base,
		{
			id: "goals",
			tabs: [
				...(goals.length ? [{ key: "event:goal", label: "goals" }] : []),
				...(propKeys.length ? [{ key: "props", label: "props" }] : [])
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

	$effect(() => {
		api<{ goals: { name: string }[] }>(`/api/app/sites/${domain}/goals`)
			.then((result) => (goals = result.goals.map((goal) => goal.name)))
			.catch(() => (goals = []));
	});

	$effect(() => {
		void version;
		if (!dates) return;
		const range = dates.current === "30m" ? "day" : dates.current;
		api<string[]>("/api/app/props", { site_id: domain, date_range: range }).then((keys) => {
			propKeys = keys;
			if (!keys.includes(propKey)) propKey = keys[0] ?? "";
		});
	});
	let ranked = $state<Record<string, Ranked>>({});
	let loading = $state<Record<string, boolean>>({});

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
		goto(resolve(`/[site]?${params}`, { site: domain }), { reset: false });
	}

	let editing = $state<Filter | null | undefined>(undefined);
	let details = $state<{ dim: string; title: string } | null>(null);
	const converting = $derived(filters.some((filter) => filter.dim === "event:goal"));
	const dimOf = (id: string) => (tabs[id] === "props" ? `event:props:${propKey}` : tabs[id]);

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
		api<{ segments: Segment[] }>(`/api/app/sites/${domain}/segments`)
			.then((result) => (segments = result.segments))
			.catch(() => (segments = []));
	});

	function pickSegment(key: string) {
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
		if (grain === "time:hour" || grain === "time:minute") return raw.slice(11, 16);
		const options: Intl.DateTimeFormatOptions =
			grain === "time:month"
				? { month: "short", year: "2-digit" }
				: { month: "short", day: "numeric" };
		return date.toLocaleDateString("en-US", options);
	}

	$effect(() => {
		void version;
		if (!dates) return;
		const where = toQueryFilters(filters);
		const keys = METRICS.map((item) => item.key);
		let stale = false;
		const { previous: before, grain } = dates;
		Promise.all([
			ask({ metrics: keys, date_range: dates.current, filters: where }),
			before && ask({ metrics: keys, date_range: before, filters: where }),
			ask({
				metrics: [chartMetric],
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
			quiet = !now.results[0]?.metrics[0];
			hasImports =
				!withImports || (now.meta?.imports_skip_reason ?? "") !== "no_imported_data";
			totals = now.results[0]?.metrics ?? null;
			previousTotals = before ? (before.results[0]?.metrics ?? null) : null;
			series = line.results.map((row) => row.metrics[0]);
			previousSeries = previousLine ? previousLine.results.map((row) => row.metrics[0]) : [];
			labels = line.results.map((row) => label(row.dimensions[0], grain));
		});
		return () => (stale = true);
	});

	$effect(() => {
		void version;
		if (!dates) return;
		const where = toQueryFilters(filters);
		const cleanups: (() => void)[] = [];
		for (const panel of panels) {
			if (!panel.tabs.length) continue;
			const dim = tabs[panel.id] === "props" ? `event:props:${propKey}` : tabs[panel.id];
			let stale = false;
			loading[panel.id] = true;
			ask({
				metrics:
					dim === "event:goal" || converting
						? ["visitors", "conversion_rate"]
						: ["visitors"],
				date_range: dates.current,
				dimensions: [dim],
				filters: where,
				pagination: { limit: 9 }
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
						...(PAGES.has(dim) &&
							value.startsWith("/") && { href: `https://${domain}${value}` }),
						...(ICONIC.has(dim) && {
							icon: icon(dim, value),
							fallback: LETTERED.has(dim) ? label : undefined
						})
					};
				});
				loading[panel.id] = false;
			});
			cleanups.push(() => (stale = true));
		}
		return () => cleanups.forEach((cleanup) => cleanup());
	});
</script>

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
			iconic={ICONIC.has(details.dim)}
			pick={(value) => {
				const dim = details?.dim ?? "";
				details = null;
				addFilter(dim, value);
			}}
		/>
	{/if}
</Dialog>

{#snippet picker()}
	<Select
		align="right"
		value={propKey}
		options={propKeys.map((key) => ({ key, label: key }))}
		data
		pick={(key) => (propKey = key)}
	/>
{/snippet}

<Shell>
	{#snippet actions()}
		<Live site={domain} />
	{/snippet}

	<div class="flex flex-col gap-6">
		<div class="flex flex-wrap items-center gap-3">
			<Select
				value={domain}
				options={sites}
				data
				pick={(key) => goto(resolve("/[site]", { site: key }))}
			/>
			{#if editable}
				<a
					href={resolve("/[site]/settings", { site: domain })}
					class="grid h-9 w-9 place-items-center rounded-px border border-edge text-dim transition-colors duration-100 hover:border-dim hover:text-fg"
					aria-label="site settings"
					title="site settings"
				>
					<Icon svg={sliders} size={14} />
				</a>
			{/if}
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
			{#if !empty && (segments.length || filters.length)}
				<Select
					value={active ? String(active.id) : ""}
					text={active?.name ?? "segments"}
					icon={bookmark}
					data
					options={[
						...segments.map((segment) => ({
							key: String(segment.id),
							label: segment.name
						})),
						...(filters.length && !active
							? [{ key: "save", label: "save as segment" }]
							: [])
					]}
					pick={pickSegment}
				/>
			{/if}
			{#if !empty}
				<Button variant="quiet" onclick={() => (editing = null)}>
					<Icon svg={funnel} size={12} />
					filter
				</Button>
			{/if}
			<div class="ml-auto flex flex-wrap items-center gap-3">
				{#if hasImports}
					<Checks
						bind:value={
							() => (withImports ? ["imports"] : []),
							(value) =>
								navigate((params) =>
									value.length
										? params.delete("imports")
										: params.set("imports", "0")
								)
						}
						options={[{ key: "imports", label: "imported data" }]}
					/>
				{/if}
				{#if view && !empty}
					<Period {view} {go} />
				{/if}
			</div>
		</div>

		{#if empty}
			<section class="flex max-w-2xl flex-col gap-5">
				<Snippet {domain} />
				<Message
					>Add this to your site's head. The dashboard opens with the first visit.</Message
				>
			</section>
		{:else}
			<section class="rounded-px border border-line bg-panel">
				<div class="grid grid-cols-2 border-b border-line sm:grid-cols-3 lg:grid-cols-6">
					{#each METRICS as item, i (item.key)}
						<Kpi
							label={item.label}
							value={totals?.[i] ?? null}
							change={totals && previousTotals
								? delta(item.key, totals[i], previousTotals[i])
								: null}
							unit={item.key === "bounce_rate" ? "pt" : "%"}
							format={(value) => format(item.key, value)}
							active={chartMetric === item.key}
							pick={() => navigate((params) => params.set("metric", item.key))}
						/>
					{/each}
				</div>
				<div class="relative px-4 pt-8 pb-5">
					{#if dates && dates.choices.length > 1}
						<div class="absolute top-3 right-4 z-10">
							<Select
								align="right"
								value={dates.interval}
								options={dates.choices.map((key) => ({ key, label: key }))}
								pick={(key) => go({ interval: key })}
							/>
						</div>
					{/if}
					<Chart
						values={series}
						previous={previousSeries}
						{labels}
						format={(value) => format(chartMetric, value)}
					/>
				</div>
			</section>

			<div class="grid gap-6 md:grid-cols-2">
				{#each panels.filter((panel) => panel.tabs.length) as panel (panel.id)}
					<div class={panel.id === "goals" ? "md:col-span-2" : ""}>
						<Panel
							tabs={panel.tabs}
							bind:tab={tabs[panel.id]}
							actions={tabs[panel.id] === "props" ? picker : undefined}
						>
							{#snippet footer()}
								<Button
									variant="plain"
									onclick={() =>
										(details = {
											dim: dimOf(panel.id),
											title:
												panel.tabs.find((tab) => tab.key === tabs[panel.id])
													?.label ?? ""
										})}
								>
									details
								</Button>
							{/snippet}
							<Rows
								rows={ranked[panel.id] ?? null}
								loading={loading[panel.id]}
								format={(value) => format("visitors", value)}
								pick={(key) => addFilter(dimOf(panel.id), key)}
							/>
						</Panel>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</Shell>
