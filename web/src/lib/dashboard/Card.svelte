<script lang="ts">
	import { untrack } from "svelte";
	import { api } from "#lib/api.ts";
	import WorldMap from "#lib/dither/Map.svelte";
	import { ICONIC, LETTERED, icon } from "#lib/icons/resolve.ts";
	import { cached, display, format, toQueryFilters } from "#lib/stats.ts";
	import Empty from "#lib/ui/Empty.svelte";
	import Panel from "#lib/ui/Panel.svelte";
	import Rows from "#lib/ui/Rows.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Flow from "./Flow.svelte";
	import { PAGES, UTM, drilled } from "./panels.ts";
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
	type Where = ReturnType<typeof toQueryFilters>;

	let chosen = $state("");
	let utm = $state("visit:utm_campaign");
	let chosenProp = $state("");
	let chosenFunnel = $state(0);
	let ranked = $state.raw<Ranked>(null);
	let loading = $state(false);
	let failed = $state(false);
	let tries = $state(0);
	// search console terms replace referrers under the google source when connected
	let terms = $state(false);
	// the dimension the rows on screen belong to
	let shown = $state("");

	const tab = $derived(
		panel.tabs.some((item) => item.key === chosen) ? chosen : panel.tabs[0].key
	);
	const prop = $derived(ctx.propKeys.includes(chosenProp) ? chosenProp : (ctx.propKeys[0] ?? ""));
	const funnel = $derived(
		ctx.funnels.some((item) => item.id === chosenFunnel)
			? chosenFunnel
			: (ctx.funnels[0]?.id ?? 0)
	);
	const dimOf = (key: string) =>
		key === "props"
			? `event:props:${prop}`
			: key === "utm"
				? utm
				: key === "map"
					? "visit:country"
					: (drilled(ctx.filters, key)?.dim ?? key);
	const dim = $derived(dimOf(tab));
	const goalView = $derived(dim === "event:goal" || ctx.converting);
	const searching = $derived.by(() => {
		const source = ctx.filters.find((item) => item.dim === "visit:source");
		return (
			!ctx.combined &&
			tab === "visit:source" &&
			source?.op === "is" &&
			source.values.length === 1 &&
			source.values[0] === "Google"
		);
	});
	const key = $derived(
		JSON.stringify([
			ctx.domain,
			ctx.version,
			ctx.dates.current,
			ctx.filters,
			ctx.imports,
			ctx.converting,
			tab,
			dim,
			funnel,
			tries
		])
	);

	function request(at: string, key: string, where: Where) {
		return cached(
			ctx.domain,
			{
				metrics:
					at === "event:goal" || ctx.converting
						? ["visitors", "conversion_rate"]
						: ["visitors"],
				date_range: ctx.dates.current,
				dimensions: [at],
				filters: where,
				pagination: { limit: key === "map" ? 300 : 9 }
			},
			ctx.imports
		);
	}

	function regular(where: Where) {
		return request(dim, tab, where).then((result) =>
			result.results.map((row) => {
				const value = row.dimensions[0];
				const label = display(dim, value);
				return {
					key: value,
					label,
					value: row.metrics[0],
					...(goalView && { extra: `${row.metrics[1]}%` }),
					...(!ctx.combined &&
						PAGES.has(dim) &&
						value.startsWith("/") && { href: `https://${ctx.domain}${value}` }),
					...(ICONIC.has(dim) && {
						icon: icon(dim, value),
						fallback: LETTERED.has(dim) ? label : undefined
					})
				};
			})
		);
	}

	async function keywords() {
		const page = ctx.filters.find(
			(item) => item.dim === "event:page" && item.op === "is" && item.values.length === 1
		)?.values[0];
		const result = await api<{
			keywords: { name: string; visitors: number; position: number }[];
		}>("/api/app/keywords", {
			site_id: ctx.domain,
			date_range: ctx.dates.current === "30m" ? "day" : ctx.dates.current,
			page
		});
		return result.keywords.map((keyword) => ({
			key: keyword.name,
			label: keyword.name,
			value: keyword.visitors,
			extra: `#${Math.round(keyword.position)}`
		}));
	}

	async function steps(where: Where) {
		const result = await api<{ steps: { name: string; visitors: number }[] }>(
			"/api/app/funnel",
			{
				site_id: ctx.domain,
				funnel,
				date_range: ctx.dates.current === "30m" ? "day" : ctx.dates.current,
				filters: where
			}
		);
		const first = result.steps[0]?.visitors || 1;
		return result.steps.map((step, i) => ({
			key: String(i),
			label: `${i + 1}. ${step.name}`,
			value: step.visitors,
			extra: `${Math.round((step.visitors / first) * 1000) / 10}%`
		}));
	}

	function load() {
		if (tab === "flow") return;
		let stale = false;
		const at = dim;
		const where = toQueryFilters(ctx.filters);
		loading = true;
		failed = false;
		const done = (rows: Ranked, searched = false) => {
			if (stale) return;
			ranked = rows;
			terms = searched;
			shown = at;
			loading = false;
		};
		const request =
			tab === "funnels"
				? steps(where).then((rows) => done(rows))
				: searching
					? keywords()
							.then((rows) => done(rows, true))
							.catch(() => regular(where).then((rows) => done(rows)))
					: regular(where).then((rows) => done(rows));
		request.catch(() => {
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

	const tabs = $derived(
		panel.tabs.map((item) => {
			const below = drilled(ctx.filters, item.key);
			return { ...item, label: below ? (terms ? "search terms" : below.label) : item.label };
		})
	);
	const rows = $derived(
		PAGES.has(dim)
			? (ranked?.map((row) => ({ ...row, note: ctx.titles[row.key] })) ?? null)
			: ranked
	);
	const columns = $derived(
		tab === "funnels"
			? ["rate", "visitors"]
			: terms
				? ["rank", "visitors"]
				: goalView
					? ["CR", "visitors"]
					: ["visitors"]
	);
	const pick = $derived(
		tab === "funnels" || terms
			? undefined
			: (value: string) => shown === dim && ctx.add(dim, value)
	);
</script>

{#snippet picker()}
	{#if tab === "props"}
		<Select
			align="right"
			value={prop}
			options={ctx.propKeys.map((item) => ({ key: item, label: item }))}
			data
			pick={(item) => (chosenProp = item)}
		/>
	{:else if tab === "utm"}
		<Select align="right" value={utm} options={UTM} pick={(item) => (utm = item)} />
	{:else if tab === "funnels"}
		<Select
			align="right"
			value={funnel}
			options={ctx.funnels.map((item) => ({ key: item.id, label: item.name }))}
			data
			pick={(id) => (chosenFunnel = id)}
		/>
	{/if}
{/snippet}

<Panel
	{arranging}
	fit={panel.id === "goals"}
	{hide}
	{move}
	{tabs}
	warm={(key) => {
		if (key !== "funnels" && key !== "flow")
			request(dimOf(key), key, toQueryFilters(ctx.filters)).catch(() => {});
	}}
	bind:tab={() => tab, (next) => (chosen = next)}
	expand={tab === "funnels" || tab === "flow"
		? undefined
		: () => ctx.expand(dim, panel.tabs.find((item) => item.key === tab)?.label ?? "")}
	actions={["props", "utm", "funnels"].includes(tab) ? picker : undefined}
>
	{#if tab === "flow"}
		<Flow {ctx} />
	{:else if failed}
		<Empty retry={() => tries++} />
	{:else if tab === "map"}
		<div class="flex h-full items-center px-2 py-1">
			<WorldMap
				values={Object.fromEntries(
					(shown === "visit:country" ? (ranked ?? []) : []).map((row) => [
						row.key,
						row.value
					])
				)}
				label={(code) => display("visit:country", code)}
				format={(value) => format("visitors", value)}
				pick={(code) => ctx.add("visit:country", code)}
			/>
		</div>
	{:else}
		<Rows {rows} {loading} format={(value) => format("visitors", value)} {columns} {pick} />
	{/if}
</Panel>
