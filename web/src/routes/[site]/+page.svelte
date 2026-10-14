<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import Chart from "#lib/dither/Chart.svelte";
	import { session } from "#lib/session.svelte.ts";
	import {
		METRICS,
		PERIODS,
		decodeFilter,
		delta,
		display,
		encodeFilter,
		format,
		query,
		range,
		toQueryFilters,
		type Filter,
		type Metric,
		type Period
	} from "#lib/stats.ts";
	import Chip from "#lib/ui/Chip.svelte";
	import Kpi from "#lib/ui/Kpi.svelte";
	import Live from "#lib/ui/Live.svelte";
	import Panel from "#lib/ui/Panel.svelte";
	import Rows from "#lib/ui/Rows.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Shell from "#lib/ui/Shell.svelte";

	type Ranked = { key: string; label: string; value: number }[] | null;

	const domain = $derived(page.params.site ?? "");
	const site = $derived(
		session.me?.teams.flatMap((team) => team.sites).find((site) => site.domain === domain)
	);
	const sites = $derived(
		(session.me?.teams.flatMap((team) => team.sites) ?? []).map((site) => ({
			key: site.domain,
			label: site.domain
		}))
	);
	const period = $derived((page.url.searchParams.get("period") ?? "30d") as Period);
	const metric = $derived((page.url.searchParams.get("metric") ?? "visitors") as Metric);
	const filters = $derived(
		page.url.searchParams
			.getAll("f")
			.map(decodeFilter)
			.filter((filter): filter is Filter => filter !== null)
	);
	const dates = $derived(site ? range(period, site.timezone) : null);

	let totals = $state<number[] | null>(null);
	let previousTotals = $state<number[] | null>(null);
	let series = $state<number[]>([]);
	let previousSeries = $state<number[]>([]);
	let labels = $state<string[]>([]);

	const panels = [
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
	let tabs = $state<Record<string, string>>(
		Object.fromEntries(panels.map((panel) => [panel.id, panel.tabs[0].key]))
	);
	let ranked = $state<Record<string, Ranked>>({});
	let loading = $state<Record<string, boolean>>({});

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
		if (!dates) return;
		const where = toQueryFilters(filters);
		const keys = METRICS.map((item) => item.key);
		let stale = false;
		Promise.all([
			query(domain, { metrics: keys, date_range: dates.current, filters: where }),
			query(domain, { metrics: keys, date_range: dates.previous, filters: where }),
			query(domain, {
				metrics: [metric],
				date_range: dates.current,
				dimensions: [dates.grain],
				filters: where
			}),
			query(domain, {
				metrics: [metric],
				date_range: dates.previous,
				dimensions: [dates.grain],
				filters: where
			})
		]).then(([now, before, line, previousLine]) => {
			if (stale) return;
			totals = now.results[0]?.metrics ?? null;
			previousTotals = before.results[0]?.metrics ?? null;
			series = line.results.map((row) => row.metrics[0]);
			previousSeries = previousLine.results.map((row) => row.metrics[0]);
			labels = line.results.map((row) => label(row.dimensions[0], dates.grain));
		});
		return () => (stale = true);
	});

	$effect(() => {
		if (!dates) return;
		const where = toQueryFilters(filters);
		const cleanups: (() => void)[] = [];
		for (const panel of panels) {
			const dim = tabs[panel.id];
			let stale = false;
			loading[panel.id] = true;
			query(domain, {
				metrics: ["visitors"],
				date_range: dates.current,
				dimensions: [dim],
				filters: where,
				pagination: { limit: 9 }
			}).then((result) => {
				if (stale) return;
				ranked[panel.id] = result.results.map((row) => ({
					key: row.dimensions[0],
					label: display(dim, row.dimensions[0]),
					value: row.metrics[0]
				}));
				loading[panel.id] = false;
			});
			cleanups.push(() => (stale = true));
		}
		return () => cleanups.forEach((cleanup) => cleanup());
	});
</script>

<Shell>
	{#snippet actions()}
		<Live site={domain} />
		<Select
			value={period}
			options={PERIODS.map((item) => ({ key: item.key, label: item.label }))}
			pick={(key) => navigate((params) => params.set("period", key))}
		/>
	{/snippet}

	<div class="flex flex-col gap-6">
		<div class="flex flex-wrap items-center gap-3">
			<Select
				value={domain}
				options={sites}
				pick={(key) => goto(resolve("/[site]", { site: key }))}
			/>
			{#each filters as filter (encodeFilter(filter))}
				<Chip
					label={filter.dim.split(":").at(-1) ?? filter.dim}
					value={display(filter.dim, filter.value)}
					remove={() => removeFilter(filter)}
				/>
			{/each}
		</div>

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
			<div class="px-4 pt-8 pb-5">
				<Chart
					values={series}
					previous={previousSeries}
					{labels}
					format={(value) => format(metric, value)}
				/>
			</div>
		</section>

		<div class="grid gap-6 md:grid-cols-2">
			{#each panels as panel (panel.id)}
				<Panel tabs={panel.tabs} bind:tab={tabs[panel.id]}>
					<Rows
						rows={ranked[panel.id] ?? null}
						loading={loading[panel.id]}
						format={(value) => format("visitors", value)}
						pick={(key) => addFilter(tabs[panel.id], key)}
					/>
				</Panel>
			{/each}
		</div>
	</div>
</Shell>
