<script lang="ts">
	import { untrack } from "svelte";
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { api, get } from "#lib/api.ts";
	import { density, paint } from "#lib/dither/heat.ts";
	import Period from "#lib/dashboard/Period.svelte";
	import Sections from "#lib/dashboard/Sections.svelte";
	import SitePicker from "#lib/dashboard/SitePicker.svelte";
	import Unrecorded from "#lib/dashboard/Unrecorded.svelte";
	import { current, parse } from "#lib/period.ts";
	import { findSite } from "#lib/session.svelte.ts";
	import { saveSite } from "#lib/settings/site.ts";
	import { cached, compact } from "#lib/stats.ts";
	import { theme } from "#lib/theme.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Empty from "#lib/ui/Empty.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import Frame from "./Frame.svelte";

	let { domain, timezone }: { domain: string; timezone: string } = $props();

	type Screen = "desktop" | "tablet" | "mobile";
	type Size = { width: number; height: number; updated_at: number };

	const view = $derived(parse(page.url.searchParams, timezone));
	const range = $derived(view ? current(view) : "28d");
	const on = $derived(!findSite(domain)?.skip.includes("heatmaps"));
	const editable = $derived(["owner", "admin", "editor"].includes(findSite(domain)?.role ?? ""));
	const screen = $derived(
		(["desktop", "tablet", "mobile"] as const).find(
			(item) => item === page.url.searchParams.get("screen")
		) ?? "desktop"
	);

	let pages = $state.raw<{ key: string; label: string }[] | null>(null);
	let clicks = $state.raw<number[][]>([]);
	let size = $state.raw<Size | null>(null);
	let node = $state.raw<unknown>(null);
	let scrolled = $state<number | null>(null);
	let loading = $state(true);
	let failed = $state(false);
	let tries = $state(0);
	let box = $state(0);
	let canvas = $state<HTMLCanvasElement>();

	const chosen = $derived(page.url.searchParams.get("page") ?? pages?.[0]?.key ?? "");
	const scale = $derived(size ? Math.min(1, box / size.width) : 1);
	const total = $derived(clicks.reduce((sum, click) => sum + click[2], 0));

	function go(to: Record<string, string | null>) {
		const kept = [...page.url.searchParams].filter(([key]) => !(key in to));
		const set = Object.entries(to).filter((pair): pair is [string, string] => pair[1] !== null);
		// the path is this route, already resolved
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		goto(`${page.url.pathname}?${new URLSearchParams([...kept, ...set])}`, { reset: false });
	}

	const date_range = $derived(range === "30m" ? "day" : range);

	$effect(() => {
		void tries;
		if (!on) return;
		let stale = false;
		cached(
			domain,
			{
				metrics: ["events"],
				date_range,
				dimensions: ["event:page"],
				filters: [["is", "event:name", ["~click"]]],
				pagination: { limit: 30 }
			},
			false
		)
			.then((result) => {
				if (stale) return;
				pages = result.results.map((row) => ({
					key: row.dimensions[0],
					label: row.dimensions[0]
				}));
				if (!pages.length) loading = false;
			})
			.catch(() => {
				if (!stale) failed = true;
			});
		return () => (stale = true);
	});

	$effect(() => {
		const target = chosen;
		void tries;
		if (!on || !target) return;
		let stale = false;
		loading = true;
		failed = false;
		untrack(() => {
			Promise.all([
				api<{ clicks: number[][]; snapshot: Size | null }>("/api/app/heatmap", {
					site_id: domain,
					date_range,
					page: target,
					class: screen
				}),
				cached(domain, {
					metrics: ["scroll_depth"],
					date_range,
					filters: [["is", "event:page", [target]]]
				})
			])
				.then(async ([result, scroll]) => {
					const snapshot = result.snapshot
						? await get<{ node: unknown }>(
								`/api/app/sites/${domain}/snapshot?page=${encodeURIComponent(target)}&class=${screen}`
							)
						: null;
					if (stale) return;
					clicks = result.clicks;
					size = result.snapshot;
					node = snapshot?.node ?? null;
					scrolled = (scroll.results[0]?.metrics[0] as number | null) ?? null;
					loading = false;
				})
				.catch(() => {
					if (stale) return;
					failed = true;
					loading = false;
				});
		});
		return () => (stale = true);
	});

	$effect(() => {
		const ctx = canvas?.getContext("2d");
		void theme.style;
		void theme.shown;
		if (!ctx || !size || !canvas) return;
		const heat = density(clicks, size.width, size.height);
		canvas.width = heat.columns;
		canvas.height = heat.rows;
		paint(ctx, heat, theme.style === "plain");
	});
</script>

<div class="flex flex-col gap-6">
	<div class="flex flex-wrap items-center gap-3">
		<SitePicker {domain} section="heatmaps" />
		<Sections {domain} />
		{#if view && on}
			<div class="flex items-center gap-2 max-sm:w-full sm:ml-auto">
				<Period {view} go={(change) => go(change)} warm={() => {}} />
			</div>
		{/if}
	</div>

	{#if !on}
		<section class="rounded-px border border-line bg-panel">
			<Empty text="Heatmaps are off.">
				{#if editable}
					<Button
						variant="quiet"
						onclick={() =>
							saveSite(domain, {
								skip:
									findSite(domain)?.skip.filter((key) => key !== "heatmaps") ?? []
							})}>Turn on</Button
					>
				{/if}
			</Empty>
		</section>
	{:else if failed}
		<section class="rounded-px border border-line bg-panel">
			<Empty retry={() => tries++} />
		</section>
	{:else}
		<div class="flex flex-wrap items-center gap-3">
			{#if pages === null}
				<Skeleton class="h-control w-48" />
			{:else if pages.length}
				<Select
					align="left"
					value={chosen}
					options={pages.map((item) => ({ key: item.key, label: item.label }))}
					pick={(key) => go({ page: key })}
				/>
			{/if}
			<Toggle
				bind:value={() => screen, (next: Screen) => go({ screen: next })}
				options={[
					{ key: "desktop", label: "desktop" },
					{ key: "tablet", label: "tablet" },
					{ key: "mobile", label: "mobile" }
				]}
			/>
			{#if size && !loading}
				<span class="label sm:ml-auto">{compact(total)} clicks</span>
			{/if}
		</div>

		<section
			bind:clientWidth={box}
			class="relative overflow-hidden rounded-px border border-line bg-panel"
		>
			{#if pages?.length === 0}
				<Unrecorded {domain} text="No clicks yet." />
			{:else if loading && !size}
				<Skeleton class="h-[60vh] w-full" />
			{:else if !size || !node}
				<Empty text="No snapshot of this page yet." />
			{:else}
				<div
					class="relative mx-auto"
					style:width="{size.width * scale}px"
					style:height="{size.height * scale}px"
				>
					{#key node}
						<Frame
							{node}
							href="https://{domain}{chosen}"
							width={size.width}
							height={size.height}
							{scale}
						/>
					{/key}
					<canvas
						bind:this={canvas}
						class="absolute inset-0 h-full w-full {theme.style === 'plain'
							? ''
							: 'pixelated'}"
						aria-label="{compact(total)} clicks"
					></canvas>
					{#if scrolled}
						<div
							class="absolute inset-x-0 border-t border-dashed border-fg/60"
							style:top="{(scrolled / 100) * size.height * scale}px"
						>
							<span
								class="absolute top-1 right-2 rounded-px bg-ink/80 px-1.5 py-1 label text-fg"
								>{Math.round(scrolled)}% scrolled</span
							>
						</div>
					{/if}
				</div>
			{/if}
		</section>
	{/if}
</div>
