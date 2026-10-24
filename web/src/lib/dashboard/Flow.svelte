<script lang="ts">
	import { untrack } from "svelte";
	import login from "pixelarticons/svg/login.svg?raw";
	import logout from "pixelarticons/svg/logout.svg?raw";
	import { cached, format, toQueryFilters } from "#lib/stats.ts";
	import Empty from "#lib/ui/Empty.svelte";
	import Rows from "#lib/ui/Rows.svelte";
	import Trail from "#lib/ui/Trail.svelte";
	import type { Context } from "./types.ts";

	let { ctx, limit = 7 }: { ctx: Context; limit?: number } = $props();

	type Ranked = NonNullable<Parameters<typeof Rows>[1]["rows"]>;
	type Result = Awaited<ReturnType<typeof cached>>;

	const ENTRANCE = "(entrance)";
	const EXIT = "(exit)";

	const filtered = $derived(
		ctx.filters.find(
			(item) => item.dim === "event:page" && item.op === "is" && item.values.length === 1
		)?.values[0]
	);
	// pages in the order visitors walk them, the one at `at` is in focus
	let walked = $state<string[]>([]);
	let at = $state(0);
	let top = $state("");
	let before = $state.raw<Ranked | null>(null);
	let after = $state.raw<Ranked | null>(null);
	let loading = $state(false);
	let failed = $state(false);
	let tries = $state(0);

	const path = $derived(walked.length ? walked : [filtered ?? top].filter(Boolean));
	const focus = $derived(path[at] ?? "");
	const others = $derived(ctx.filters.filter((item) => item.dim !== "event:page"));
	const key = $derived(
		JSON.stringify([ctx.domain, ctx.version, ctx.dates.current, others, focus, tries])
	);

	$effect(() => {
		void filtered;
		walked = [];
		at = 0;
	});

	const ask = (metric: string, dimensions: string[], extra: unknown[]) =>
		cached(
			ctx.domain,
			{
				metrics: [metric],
				date_range: ctx.dates.current,
				dimensions,
				filters: [...toQueryFilters(others), ...extra],
				pagination: { limit }
			},
			false
		);

	const pages = (result: Result): Ranked =>
		result.results.map((row) => ({
			key: row.dimensions[0],
			label: row.dimensions[0],
			value: row.metrics[0],
			note: ctx.titles[row.dimensions[0]]
		}));

	const edge = (key: string, label: string, svg: string, result: Result): Ranked => {
		const value = result.results[0]?.metrics[0] ?? 0;
		return value ? [{ key, label, value, icon: { kind: "pixel", svg } }] : [];
	};

	const ranked = (rows: Ranked) => rows.sort((a, b) => b.value - a.value).slice(0, limit);

	function load() {
		let stale = false;
		loading = true;
		failed = false;
		const page = focus;
		const viewed = ["is", "event:name", ["pageview"]];
		const work = page
			? Promise.all([
					ask("pageviews", ["event:props:~from"], [viewed, ["is", "event:page", [page]]]),
					ask("visits", [], [["is", "visit:entry_page", [page]]]),
					ask("pageviews", ["event:page"], [viewed, ["is", "event:props:~from", [page]]]),
					ask("visits", [], [["is", "visit:exit_page", [page]]])
				]).then(([from, entrances, to, exits]) => {
					if (stale) return;
					before = ranked([
						...edge(ENTRANCE, "entrance", login, entrances),
						...pages(from)
					]);
					after = ranked([...pages(to), ...edge(EXIT, "exit", logout, exits)]);
				})
			: ask("pageviews", ["event:page"], [viewed]).then((result) => {
					if (!stale) top = result.results[0]?.dimensions[0] ?? "";
				});
		work.then(() => {
			if (!stale) loading = false;
		}).catch(() => {
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

	function walk(page: string, forward: boolean) {
		if (page === ENTRANCE || page === EXIT) return;
		const kept = forward ? path.slice(0, at + 1) : path.slice(at);
		walked = forward ? [...kept, page] : [page, ...kept];
		at = forward ? walked.length - 1 : 0;
	}
</script>

{#if failed}
	<Empty retry={() => tries++} />
{:else}
	<div class="flex flex-col gap-2">
		<Trail items={path} current={at} pick={(i) => (at = i)} />
		<div class="grid grid-cols-2 gap-2">
			<Rows
				rows={before}
				{loading}
				{limit}
				format={(value) => format("pageviews", value)}
				columns={["from"]}
				pick={(page) => walk(page, false)}
			/>
			<Rows
				rows={after}
				{loading}
				{limit}
				format={(value) => format("pageviews", value)}
				columns={["next"]}
				pick={(page) => walk(page, true)}
			/>
		</div>
	</div>
{/if}
