<script lang="ts">
	import github from "pixelarticons/svg/github.svg?raw";
	import star from "pixelarticons/svg/star.svg?raw";
	import Meteor from "#lib/art/Meteor.svelte";
	import Chart from "#lib/dither/Chart.svelte";
	import Button from "#lib/ui/Button.svelte";
	import Logo from "#lib/ui/Logo.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Rows from "#lib/ui/Rows.svelte";
	import Kpis from "#lib/ui/Kpis.svelte";
	import Panel from "#lib/ui/Panel.svelte";
	import { Tween } from "svelte/motion";
	import { ICONIC, LETTERED, icon } from "#lib/icons/resolve.ts";
	import { METRICS, format } from "#lib/stats.ts";
	import {
		BENCH,
		DEMO,
		FEATURES,
		GOATCOUNTER,
		LABELS,
		MARKS,
		PANELS,
		PLAUSIBLE,
		PREVIOUS,
		REPO,
		STATS,
		TOOLS,
		TOTALS,
		shape
	} from "../lib/demo.ts";
	import Edit from "../lib/Edit.svelte";
	import Install from "../lib/Install.svelte";
	import Race from "../lib/Race.svelte";
	import Scale from "../lib/Scale.svelte";
	import Section from "../lib/Section.svelte";
	import Sprite from "../lib/Sprite.svelte";
	import Surface from "../lib/Surface.svelte";
	import Ticker from "../lib/Ticker.svelte";
	import { CRATE, CRATER, RACK } from "../lib/shapes.ts";
	import { appear } from "../lib/appear.ts";
	import { impact } from "../lib/impact.ts";

	let { data } = $props();

	const compact = (value: number) => Math.round(value).toLocaleString("en-US");
	const stat = (label: string) => STATS.find((item) => item.label === label)?.values[0] ?? 0;

	let landed = $state(true);
	let pageviews = $state(50);
	let moved = $state(false);
	const heat = new Tween(0.2, { duration: 1400 });
	const flare = () => heat.set(1, { duration: 0 }).then(() => heat.set(0.2));
	let metric = $state(0);
	let tabs = $state({ left: "visit:source", right: "event:page" });
	const local = (spec: ReturnType<typeof icon>) =>
		spec?.kind === "image"
			? { ...spec, url: spec.url.replace("/api/app/icon/", "/icons/") + ".png" }
			: spec;
	const rowsOf = (side: "left" | "right") =>
		PANELS[side].rows[tabs[side]].map(([label, value]) => ({
			key: label,
			label,
			value: landed ? value : 0,
			...(ICONIC.has(tabs[side]) && {
				icon: local(icon(tabs[side], label)),
				fallback: LETTERED.has(tabs[side]) ? label : undefined
			})
		}));
</script>

<svelte:head>
	<link rel="prefetch" href={DEMO} />
</svelte:head>

<div id="top" class="flex flex-col" {@attach appear}>
	<div class="mx-auto w-full max-w-6xl gutter pb-section">
		<header class="relative z-30 flex h-16 items-center justify-between">
			<a href="#top" class="flex h-control items-center gap-2 font-mono text-body">
				<Logo />
				<span class="max-[23rem]:hidden">impactite</span>
			</a>
			<nav class="flex items-center gap-4 sm:gap-6">
				<a href="#speed" class="hidden h-control items-center label hover:text-fg sm:flex"
					>speed</a
				>
				<a
					href="#compatible"
					class="hidden h-control items-center label hover:text-fg sm:flex">compatible</a
				>
				<a href={DEMO} class="flex h-control items-center label hover:text-fg">demo</a>
				<a href="#install" class="flex h-control items-center label hover:text-fg"
					>install</a
				>
				<a
					href={REPO}
					class="flex h-control items-center gap-1.5 label hover:text-fg"
					aria-label="source on github{data.stars === null
						? ''
						: `, ${data.stars} stars`}"
				>
					<Icon svg={github} size={12} />
					{#if data.stars !== null}
						<Icon svg={star} size={12} />
						<span class="num">{compact(data.stars)}</span>
					{/if}
				</a>
			</nav>
		</header>

		<div
			class="relative flex flex-col gap-section pt-8 md:pt-12"
			data-landed="false"
			use:impact={(value) => (landed = value)}
		>
			<section class="grid items-center gap-8 md:grid-cols-[1fr_auto] md:gap-12">
				<div class="shown relative z-30 flex flex-col gap-6" data-appear>
					<h1 class="font-mono text-big tracking-tight extrude md:text-display">
						Analytics<br />that land.
					</h1>
					<p class="max-w-md text-title text-dim">
						Self-hosted web analytics in one binary. Drop-in for Plausible and
						GoatCounter.
					</p>
					<div class="flex gap-3">
						<Button href="#install">Install</Button>
						<Button href={DEMO} variant="quiet">Live demo</Button>
					</div>
				</div>
				<div class="relative z-20 w-[200px] justify-self-center md:w-[340px]" data-rock>
					<Meteor size={340} pixel={4} />
				</div>
			</section>

			<div class="flex flex-col gap-6">
				<section
					class="shown rounded-px border border-line bg-panel"
					data-appear
					style:--delay="120ms"
				>
					<Kpis
						totals={landed ? TOTALS : TOTALS.map(() => 0)}
						previous={landed ? PREVIOUS : null}
						active={metric}
						pick={(i) => (metric = i)}
					/>
					<div class="px-4 pt-3 pb-5" data-chart>
						<Chart
							values={shape(metric)}
							labels={LABELS}
							format={(value) => format(METRICS[metric].key, value)}
							height={200}
							marks={landed ? MARKS : []}
						/>
					</div>
				</section>
				<div class="grid gap-6 md:grid-cols-2" data-appear style:--delay="240ms">
					{#each ["left", "right"] as const as side (side)}
						<Panel
							tabs={PANELS[side].tabs}
							bind:tab={tabs[side]}
							expand={() => (location.href = DEMO)}
						>
							<Rows rows={rowsOf(side)} format={compact} columns={["visitors"]} />
						</Panel>
					{/each}
				</div>
			</div>
		</div>
	</div>

	<Ticker />

	<main>
		<Surface>
			<section id="speed" class="flex flex-col gap-14">
				<h2 class="font-mono text-heading tracking-tight extrude" data-appear>
					Benchmarks
				</h2>
				<div class="grid gap-x-12 gap-y-10 md:grid-cols-2">
					{#each STATS as item, i (item.label)}
						<div
							class="flex flex-col gap-4"
							data-appear
							style:--delay="{(i % 3) * 80}ms"
						>
							<span class="label">{item.label}</span>
							<Race
								unit={item.unit}
								digits={item.digits ?? 0}
								better={item.better}
								rows={TOOLS.map((label, k) => ({
									label,
									value: item.values[k],
									ours: k === 0
								}))}
							/>
						</div>
					{/each}
				</div>
				<p class="text-body text-dim" data-appear>
					1M pageviews each on one machine. Plausible CE 3.2.1, GoatCounter 2.7.0, Ryzen 5
					7600X.
					<a href={BENCH} class="-m-3 p-3 text-cyan hover:text-fg">Source</a>
				</p>
			</section>
		</Surface>

		<Surface raised>
			<Section
				id="scale"
				title="Do you really need Kubernetes?"
				note="Set your monthly pageviews to see the load on one server."
			>
				{#snippet art()}<Sprite shape={RACK} level={pageviews / 100} size={288} />{/snippet}
				<Scale
					cpu={stat("cpu for 1M pageviews")}
					rate={stat("pageviews per second")}
					bind:position={pageviews}
				/>
			</Section>
		</Surface>

		<Surface>
			<Section
				id="compatible"
				flip
				title="Migrating from Plausible or GoatCounter?"
				note="Just change the host in your script. Your old snippets, custom events and API calls keep working."
			>
				{#snippet art()}<Sprite shape={CRATE} level={moved ? 1 : 0} size={288} />{/snippet}
				<div class="flex flex-col gap-4">
					<span class="label">plausible</span>
					<Edit parts={PLAUSIBLE} delay={300} />
					<span class="label">goatcounter</span>
					<Edit parts={GOATCOUNTER} delay={1900} done={() => (moved = true)} />
				</div>
			</Section>
		</Surface>

		<Surface raised>
			<section id="features" class="flex flex-col gap-14">
				<h2 class="font-mono text-heading tracking-tight extrude" data-appear>
					Everything included.
				</h2>
				<div
					class="grid gap-px overflow-hidden rounded-px border border-line bg-line sm:grid-cols-2 lg:grid-cols-4"
				>
					{#each FEATURES as feature, i (feature.label)}
						<div
							class="flex items-center gap-3 bg-panel px-4 py-5"
							data-appear
							style:--delay="{i * 40}ms"
						>
							<Icon svg={feature.icon} size={16} class="text-dim" />
							<span class="text-body">{feature.label}</span>
						</div>
					{/each}
				</div>
			</section>
		</Surface>

		<Surface>
			<Section id="install" title="One binary. Done.">
				{#snippet art()}
					<Sprite shape={CRATER} level={heat.current} size={288} poke={flare} />
				{/snippet}
				<Install change={flare} />
			</Section>
		</Surface>
	</main>

	<div class="mx-auto w-full max-w-6xl gutter pb-8">
		<footer
			class="flex flex-col gap-4 border-t border-line py-8 sm:flex-row sm:items-center sm:justify-between"
			data-appear
		>
			<span class="flex items-center gap-2 font-mono text-small text-dim">
				<Logo size={16} />
				impactite
			</span>
			<span class="flex flex-wrap items-center gap-x-6 gap-y-2 whitespace-nowrap">
				<a href="{REPO}/blob/main/LICENSE" class="-my-4 py-4 label hover:text-fg"
					>agpl 3.0, tracker mit</a
				>
				<a href="{REPO}/blob/main/COMMERCIAL.md" class="-my-4 py-4 label hover:text-fg"
					>commercial license</a
				>
			</span>
		</footer>
	</div>
</div>
