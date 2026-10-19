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
	import { ICONIC, LETTERED, icon } from "#lib/icons/resolve.ts";
	import { METRICS, format } from "#lib/stats.ts";
	import {
		BENCH,
		DEMO,
		FEATURES,
		GOATCOUNTER,
		LABELS,
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
	import { appear } from "../lib/appear.ts";
	import { impact } from "../lib/impact.ts";

	let { data } = $props();

	const compact = (value: number) => Math.round(value).toLocaleString("en-US");
	const stat = (label: string) => STATS.find((item) => item.label === label)?.values[0] ?? 0;

	let landed = $state(true);
	let metric = $state(0);
	let tabs = $state({ left: "visit:source", right: "event:page" });
	const rowsOf = (side: "left" | "right") =>
		PANELS[side].rows[tabs[side]].map(([label, value]) => ({
			key: label,
			label,
			value: landed ? value : 0,
			...(ICONIC.has(tabs[side]) && {
				icon: icon(tabs[side], label),
				fallback: LETTERED.has(tabs[side]) ? label : undefined
			})
		}));
</script>

<svelte:head>
	<link rel="prefetch" href={DEMO} />
</svelte:head>

<div id="top" class="mx-auto flex max-w-6xl flex-col px-6" {@attach appear}>
	<header class="flex h-16 items-center justify-between">
		<a href="#top" class="flex h-control items-center gap-2 font-mono text-body">
			<Logo />
			impactite
		</a>
		<nav class="flex items-center gap-6">
			<a href="#speed" class="hidden h-control items-center label hover:text-fg sm:flex"
				>speed</a
			>
			<a href="#compatible" class="hidden h-control items-center label hover:text-fg sm:flex"
				>compatible</a
			>
			<a href={DEMO} class="flex h-control items-center label hover:text-fg">demo</a>
			<a href="#install" class="hidden h-control items-center label hover:text-fg sm:flex"
				>install</a
			>
			<a
				href={REPO}
				class="flex h-control items-center gap-1.5 label hover:text-fg"
				aria-label="source on github{data.stars === null ? '' : `, ${data.stars} stars`}"
			>
				<Icon svg={github} size={12} />
				{#if data.stars !== null}
					<Icon svg={star} size={12} />
					<span class="num">{compact(data.stars)}</span>
				{/if}
			</a>
		</nav>
	</header>

	<div class="relative" data-landed="false" use:impact={(value) => (landed = value)}>
		<section class="grid items-center gap-8 py-12 md:grid-cols-[1fr_auto] md:gap-12 md:py-20">
			<div class="shown flex flex-col gap-6" data-appear>
				<h1 class="font-mono text-big tracking-tight md:text-display">
					Analytics<br />that land.
				</h1>
				<p class="max-w-md text-title text-dim">
					Self-hosted web analytics in one binary. Drop-in for Plausible and GoatCounter.
				</p>
				<div class="flex gap-3">
					<Button href="#install">Install</Button>
					<Button href={DEMO} variant="quiet">Live demo</Button>
				</div>
			</div>
			<div class="relative z-20 justify-self-center" data-rock>
				<Meteor size={340} pixel={4} />
			</div>
		</section>

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
				/>
			</div>
		</section>
		<div class="mt-6 grid gap-6 md:grid-cols-2" data-appear style:--delay="240ms">
			{#each ["left", "right"] as const as side (side)}
				<Panel tabs={PANELS[side].tabs} bind:tab={tabs[side]}>
					<Rows rows={rowsOf(side)} format={compact} limit={5} />
				</Panel>
			{/each}
		</div>
	</div>

	<section id="speed" class="flex flex-col gap-14 py-20 md:py-28">
		<h2 class="font-mono text-heading tracking-tight" data-appear>Benchmarks</h2>
		<div class="grid gap-x-12 gap-y-10 md:grid-cols-2 lg:grid-cols-3">
			{#each STATS as item, i (item.label)}
				<div class="flex flex-col gap-4" data-appear style:--delay="{(i % 3) * 80}ms">
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
			1M pageviews each on one machine. Plausible CE 3.2.1, GoatCounter 2.7.0, Ryzen 5 7600X.
			<a href={BENCH} class="text-cyan hover:text-fg">Source</a>
		</p>
	</section>

	<Section id="scale" title="One server is enough.">
		<Scale cpu={stat("cpu for 1M pageviews")} rate={stat("pageviews per second")} />
	</Section>

	<Section
		id="compatible"
		title="Keep your snippets."
		note="Change the host. Old snippets, custom events and API calls keep working."
	>
		<div class="flex flex-col gap-4">
			<span class="label">plausible</span>
			<Edit parts={PLAUSIBLE} delay={300} />
			<span class="label">goatcounter</span>
			<Edit parts={GOATCOUNTER} delay={1900} />
		</div>
	</Section>

	<section
		class="mb-20 grid gap-px overflow-hidden rounded-px border border-line bg-line sm:grid-cols-2 md:mb-28 lg:grid-cols-4"
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
	</section>

	<Section id="install" title="One binary. Done.">
		<Install repo={REPO} />
	</Section>

	<footer
		class="flex flex-col gap-4 border-t border-line py-8 sm:flex-row sm:items-center sm:justify-between"
		data-appear
	>
		<span class="flex items-center gap-2 font-mono text-small text-dim">
			<Logo size={16} />
			impactite
		</span>
		<span class="flex flex-wrap items-center gap-x-6 gap-y-2 whitespace-nowrap">
			<a href="{REPO}/blob/main/LICENSE" class="label hover:text-fg">agpl 3.0, tracker mit</a>
			<a href="{REPO}/blob/main/COMMERCIAL.md" class="label hover:text-fg"
				>commercial license</a
			>
		</span>
	</footer>
</div>
