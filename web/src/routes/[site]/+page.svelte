<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import Chart from "#lib/dither/Chart.svelte";
	import { api } from "#lib/api.ts";
	import { icon } from "#lib/icons/resolve.ts";
	import sliders from "pixelarticons/svg/sliders.svg?raw";
	import { session } from "#lib/session.svelte.ts";
	import {
		METRICS,
		decodeFilter,
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
	import Snippet from "#lib/Snippet.svelte";
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
		api<string[]>("/api/app/props", { site_id: domain, date_range: dates.current }).then(
			(keys) => {
				propKeys = keys;
				if (!keys.includes(propKey)) propKey = keys[0] ?? "";
			}
		);
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

	function addFilter(dim: string, value: string) {
		const filter = encodeFilter({ op: "is", dim, value });
		navigate((params) => {
			if (!params.getAll("f").includes(filter)) params.append("f", filter);
		});
	}

	function removeFilter(filter: Filter) {
		const encoded = encodeFilter(filter);
		navigate((params) => {
			const rest = params.getAll("f").filter((value) => value !== encoded);
			params.delete("f");
			for (const value of rest) params.append("f", value);
		});
	}

	function label(raw: string, grain: string) {
		const date = new Date(raw.replace(" ", "T") + (raw.length === 10 ? "T00:00:00" : ""));
		if (grain === "time:hour") return raw.slice(11, 16);
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
				metrics: [metric],
				date_range: dates.current,
				dimensions: [grain],
				filters: where
			}),
			before &&
				dates.overlay &&
				ask({ metrics: [metric], date_range: before, dimensions: [grain], filters: where })
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
				metrics: dim === "event:goal" ? ["visitors", "conversion_rate"] : ["visitors"],
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
						...(dim === "event:goal" && { extra: `${row.metrics[1]}%` }),
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

{#snippet picker()}
	<Select
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
					class="grid h-8 w-8 place-items-center rounded-px text-dim transition-colors duration-100 hover:text-fg"
					aria-label="site settings"
				>
					<Icon svg={sliders} size={14} />
				</a>
			{/if}
			{#each filters as filter (encodeFilter(filter))}
				<Chip
					label={filter.dim.split(":").at(-1) ?? filter.dim}
					value={display(filter.dim, filter.value)}
					icon={ICONIC.has(filter.dim) ? icon(filter.dim, filter.value) : undefined}
					remove={() => removeFilter(filter)}
				/>
			{/each}
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
				{#if view}
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
							active={metric === item.key}
							pick={() => navigate((params) => params.set("metric", item.key))}
						/>
					{/each}
				</div>
				<div class="relative px-4 pt-8 pb-5">
					{#if dates && dates.choices.length > 1}
						<div class="absolute top-3 right-4 z-10">
							<Select
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
						format={(value) => format(metric, value)}
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
							<Rows
								rows={ranked[panel.id] ?? null}
								loading={loading[panel.id]}
								format={(value) => format("visitors", value)}
								pick={(key) =>
									addFilter(
										tabs[panel.id] === "props"
											? `event:props:${propKey}`
											: tabs[panel.id],
										key
									)}
							/>
						</Panel>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</Shell>
