<script lang="ts" module>
	import { number } from "#lib/display.svelte.ts";
	import type { Range } from "#lib/period.ts";
	import { cached, format } from "#lib/stats.ts";

	const PAGE = 100;
	// goals have no text filter, and country and language names only exist here
	const LOCAL = ["event:goal", "visit:country", "visit:language"];
	const percent = (value: number | null) => (value === null ? "-" : `${value}%`);
	const seconds = (value: number | null) =>
		value === null ? "-" : format("visit_duration", value);
	const money = (value: number | null) => (value === null ? "-" : number(value, 2));
	const count = (value: number | null) => (value === null ? "-" : format("visitors", value));

	function columnsOf(dim: string, converting: boolean, revenue: boolean) {
		const visitors = {
			key: "visitors",
			label: converting ? "conversions" : "visitors",
			format: count
		};
		if (converting || dim === "event:goal")
			return [
				visitors,
				{ key: "events", label: "events", format: count },
				{ key: "conversion_rate", label: "cr", format: percent },
				...(revenue
					? [
							{ key: "total_revenue", label: "revenue", format: money },
							{ key: "average_revenue", label: "average", format: money }
						]
					: [])
			];
		if (dim.startsWith("event:props:"))
			return [
				visitors,
				{ key: "events", label: "events", format: count },
				{ key: "percentage", label: "share", format: percent }
			];
		if (dim === "event:page")
			return [
				visitors,
				{ key: "pageviews", label: "pageviews", format: count },
				{ key: "bounce_rate", label: "bounce", format: percent },
				{ key: "time_on_page", label: "time on page", format: seconds },
				{ key: "scroll_depth", label: "scroll", format: percent },
				{ key: "exit_rate", label: "exit rate", format: percent }
			];
		if (dim === "visit:entry_page" || dim === "visit:exit_page")
			return [
				visitors,
				{
					key: "visits",
					label: dim === "visit:entry_page" ? "entrances" : "exits",
					format: count
				},
				{ key: "bounce_rate", label: "bounce", format: percent },
				{ key: "visit_duration", label: "duration", format: seconds }
			];
		return [
			visitors,
			{ key: "bounce_rate", label: "bounce", format: percent },
			{ key: "visit_duration", label: "duration", format: seconds }
		];
	}

	type Report = {
		domain: string;
		dim: string;
		range: Range;
		filters: unknown[];
		imports: boolean;
		converting: boolean;
		revenue: boolean;
	};

	function ask(
		{ domain, dim, range, filters, imports, converting, revenue }: Report,
		typed = "",
		sort: [string, "asc" | "desc"] = ["visitors", "desc"],
		limit = PAGE
	) {
		const local = LOCAL.includes(dim);
		return cached(
			domain,
			{
				metrics: columnsOf(dim, converting, revenue).map((column) => column.key),
				date_range: range,
				dimensions: [dim],
				filters: typed && !local ? [...filters, ["contains", dim, [typed]]] : filters,
				order_by: [sort],
				pagination: { limit: local ? 1000 : limit + 1 }
			},
			imports
		);
	}

	export const prefetch = (report: Report) => ask(report).catch(() => {});
</script>

<script lang="ts">
	import { icon } from "#lib/icons/resolve.ts";
	import { display } from "#lib/stats.ts";
	import Button from "#lib/ui/Button.svelte";
	import Empty from "#lib/ui/Empty.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Table from "#lib/ui/Table.svelte";

	let {
		pick,
		...report
	}: Report & {
		iconic: boolean;
		pick: (value: string) => void;
	} = $props();

	const columns = $derived(columnsOf(report.dim, report.converting, report.revenue));

	let search = $state("");
	let sort = $state<[string, "asc" | "desc"]>(["visitors", "desc"]);
	let rows = $state<
		| {
				key: string;
				label: string;
				icon?: ReturnType<typeof icon>;
				values: (number | null)[];
		  }[]
		| null
	>(null);
	let pages = $state(1);
	let failed = $state(false);
	let tries = $state(0);
	let more = $state(false);

	$effect(() => {
		const typed = search.trim();
		const limit = pages * PAGE;
		const { dim, iconic } = report;
		const local = LOCAL.includes(dim);
		const order = sort;
		void tries;
		let stale = false;
		failed = false;
		const timer = setTimeout(
			() => {
				ask(report, typed, order, limit)
					.then((result) => {
						if (stale) return;
						const all = result.results
							.map((row) => {
								const value = row.dimensions[0];
								return {
									key: value,
									label: display(dim, value),
									...(iconic && { icon: icon(dim, value) }),
									values: row.metrics as (number | null)[]
								};
							})
							.filter(
								(row) =>
									!local || row.label.toLowerCase().includes(typed.toLowerCase())
							);
						more = all.length > limit;
						rows = all.slice(0, limit);
					})
					.catch(() => {
						if (!stale) failed = true;
					});
			},
			typed ? 150 : 0
		);
		return () => {
			stale = true;
			clearTimeout(timer);
		};
	});
</script>

<div class="flex flex-col gap-4">
	<div class="w-full sm:w-72">
		<Field label="search" bind:value={search} />
	</div>
	{#if failed}
		<Empty compact retry={() => tries++} />
	{:else}
		<Table {columns} {rows} bind:sort {pick} />
	{/if}
	{#if more}
		<div class="flex justify-center">
			<Button variant="quiet" onclick={() => pages++}>Load more</Button>
		</div>
	{/if}
</div>
