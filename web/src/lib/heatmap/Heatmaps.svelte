<script lang="ts">
	import { untrack } from "svelte";
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { api, get } from "#lib/api.ts";
	import { density, paint, shade } from "#lib/dither/heat.ts";
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
	import Rows from "#lib/ui/Rows.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import Frame from "./Frame.svelte";

	let { domain, timezone }: { domain: string; timezone: string } = $props();

	type Screen = "desktop" | "tablet" | "mobile";
	type Layer = "clicks" | "scrolling";
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
	const layer = $derived<Layer>(
		page.url.searchParams.get("layer") === "scrolling" ? "scrolling" : "clicks"
	);

	let pages = $state.raw<{ key: string; label: string }[] | null>(null);
	let clicks = $state.raw<number[][]>([]);
	let elements = $state.raw<[string, number][]>([]);
	let reach = $state.raw<number[]>([]);
	let size = $state.raw<Size | null>(null);
	let node = $state.raw<unknown>(null);
	let tall = $state(0);
	let snapshot = $state.raw<Document | null>(null);
	let hovered = $state<string | null>(null);
	let loading = $state(true);
	let failed = $state(false);
	let tries = $state(0);
	let box = $state(0);
	let canvas = $state<HTMLCanvasElement>();

	const chosen = $derived(
		page.url.searchParams.get("page") ??
			pages?.find((item) => item.key === "/")?.key ??
			pages?.[0]?.key ??
			""
	);

	async function retake() {
		await api(
			`/api/app/sites/${domain}/snapshot?page=${encodeURIComponent(chosen)}&class=${screen}`,
			undefined,
			"DELETE"
		);
		size = null;
		node = null;
	}
	const scale = $derived(size ? Math.min(1, box / size.width) : 1);
	const total = $derived(clicks.reduce((sum, click) => sum + click[2], 0));

	const find = (path: string) => {
		try {
			return snapshot?.querySelector(path) ?? null;
		} catch {
			return null;
		}
	};
	const KINDS: Record<string, string> = {
		a: "Link",
		button: "Button",
		input: "Field",
		select: "Menu",
		textarea: "Text box",
		label: "Label",
		summary: "Toggle",
		img: "Image"
	};
	// recordings mask text as stars, those fall back to the kind of element and its place
	const name = (element: Element | null, path: string) => {
		const text = (
			element?.getAttribute("aria-label") ||
			element?.querySelector("img")?.alt ||
			element?.getAttribute("title") ||
			element?.textContent ||
			""
		)
			.replace(/\s+/g, " ")
			.trim();
		if (/[^*\s]/.test(text)) return text.slice(0, 80);
		const tag = element?.localName ?? path.split(">").at(-1)?.split(":")[0] ?? "";
		const kind = KINDS[tag] ?? "Element";
		if (!element || !snapshot) return kind;
		return `${kind} ${[...snapshot.querySelectorAll(tag)].indexOf(element) + 1}`;
	};
	const rows = $derived(
		elements.map(([path, count]) => ({
			key: path,
			label: name(find(path), path),
			value: count,
			extra: total ? `${Math.round((count / total) * 100)}%` : undefined
		}))
	);
	const outline = $derived.by(() => {
		const rect = hovered ? find(hovered)?.getBoundingClientRect() : null;
		return rect?.width ? rect : null;
	});
	let marker = $state<HTMLDivElement>();

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
			api<{
				clicks: number[][];
				elements: [string, number][];
				reach: number[];
				snapshot: Size | null;
			}>("/api/app/heatmap", {
				site_id: domain,
				date_range,
				page: target,
				class: screen
			})
				.then(async (result) => {
					const stored = result.snapshot
						? await get<{ node: unknown; view?: number }>(
								`/api/app/sites/${domain}/snapshot?page=${encodeURIComponent(target)}&class=${screen}`
							)
						: null;
					if (stale) return;
					clicks = result.clicks;
					elements = result.elements;
					reach = result.reach;
					size = result.snapshot;
					node = stored?.node ?? null;
					// older snapshots don't know the screen, so a typical one for the class
					tall = stored?.view || { desktop: 900, tablet: 1100, mobile: 800 }[screen];
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
		if (layer === "clicks") paint(ctx, heat, theme.style === "plain");
		else if (reach.length) shade(ctx, reach, size.width, size.height, theme.style === "plain");
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
			<Empty text="Heatmaps are off. Turn them on to see where visitors click.">
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
				bind:value={
					() => layer, (next: Layer) => go({ layer: next === "clicks" ? null : next })
				}
				options={[
					{ key: "clicks", label: "clicks" },
					{ key: "scrolling", label: "scrolling" }
				]}
			/>
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
				{#if editable}
					<Button variant="quiet" onclick={retake}>Retake snapshot</Button>
				{/if}
			{/if}
		</div>

		<div class="grid items-start gap-6 lg:grid-cols-[minmax(0,1fr)_20rem]">
			<section
				bind:clientWidth={box}
				class="relative overflow-hidden rounded-px border border-line bg-panel"
			>
				{#if pages?.length === 0}
					<Unrecorded {domain} text="No clicks yet." />
				{:else if loading && !size}
					<Skeleton class="h-[60vh] w-full" />
				{:else if !size || !node}
					<Empty
						text="No snapshot of this page yet. One is taken the next time someone visits it."
					/>
				{:else}
					<div
						class="relative mx-auto"
						style:width="{size.width * scale}px"
						style:height="{size.height * scale}px"
					>
						{#key node}
							<Frame
								{domain}
								{node}
								view={tall}
								href="https://{domain}{chosen}"
								width={size.width}
								height={size.height}
								{scale}
								ready={(page) => (snapshot = page)}
							/>
						{/key}
						<canvas
							bind:this={canvas}
							class="absolute inset-0 h-full w-full {theme.style === 'plain'
								? ''
								: 'pixelated'}"
							aria-label="{compact(total)} clicks"
						></canvas>
						{#if layer === "scrolling"}
							{#each reach.slice(0, 9) as share, i (i)}
								{#if share !== reach[i - 1]}
									<div
										class="absolute inset-x-0 border-t border-dashed border-fg/40"
										style:top="{(i + 1) * 10}%"
									>
										<span
											class="absolute top-1 right-2 rounded-px bg-ink/80 px-1.5 py-1 label text-fg"
											>{share}% got here</span
										>
									</div>
								{/if}
							{/each}
						{/if}
						{#if outline}
							<div
								bind:this={marker}
								class="pointer-events-none absolute rounded-px outline-2 outline-offset-2 outline-cyan"
								style:left="{outline.left * scale}px"
								style:top="{outline.top * scale}px"
								style:width="{outline.width * scale}px"
								style:height="{outline.height * scale}px"
							></div>
						{/if}
					</div>
				{/if}
			</section>
			{#if rows.length && size && node}
				<section class="rounded-px border border-line bg-panel py-2 lg:sticky lg:top-4">
					<Rows
						{rows}
						limit={20}
						columns={["share", "clicks"]}
						hover={(key) => (hovered = key)}
						pick={(key) => {
							hovered = key;
							requestAnimationFrame(() =>
								marker?.scrollIntoView({ block: "center", behavior: "smooth" })
							);
						}}
					/>
				</section>
			{/if}
		</div>
	{/if}
</div>
