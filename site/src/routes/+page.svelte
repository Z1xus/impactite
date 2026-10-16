<script lang="ts">
	import bell from "pixelarticons/svg/bell.svg?raw";
	import github from "pixelarticons/svg/github.svg?raw";
	import star from "pixelarticons/svg/star.svg?raw";
	import coins from "pixelarticons/svg/coins.svg?raw";
	import download from "pixelarticons/svg/download.svg?raw";
	import filter from "pixelarticons/svg/filter.svg?raw";
	import globe from "pixelarticons/svg/globe.svg?raw";
	import key from "pixelarticons/svg/key.svg?raw";
	import link from "pixelarticons/svg/link.svg?raw";
	import mail from "pixelarticons/svg/mail.svg?raw";
	import search from "pixelarticons/svg/search.svg?raw";
	import shield from "pixelarticons/svg/shield.svg?raw";
	import upload from "pixelarticons/svg/upload.svg?raw";
	import users from "pixelarticons/svg/users.svg?raw";
	import cloud from "pixelarticons/svg/cloud-server.svg?raw";
	import grid from "pixelarticons/svg/grid-2x2-2.svg?raw";
	import lock from "pixelarticons/svg/lock.svg?raw";
	import target from "pixelarticons/svg/target.svg?raw";
	import Meteor from "#lib/art/Meteor.svelte";
	import Chart from "#lib/dither/Chart.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Rows from "#lib/ui/Rows.svelte";
	import Kpis from "#lib/ui/Kpis.svelte";
	import Panel from "#lib/ui/Panel.svelte";
	import { ICONIC, LETTERED, icon } from "#lib/icons/resolve.ts";
	import { METRICS, format } from "#lib/stats.ts";
	import Edit from "../lib/Edit.svelte";
	import Install from "../lib/Install.svelte";
	import Race from "../lib/Race.svelte";
	import Scale from "../lib/Scale.svelte";
	import { appear } from "../lib/appear.ts";
	import { reveal } from "../lib/reveal.ts";
	import { impact } from "../lib/impact.ts";

	let { data } = $props();

	const REPO = "https://github.com/Z1xus/impactite";
	const BENCH = "https://github.com/Z1xus/impactite-benchmarks";
	const DEMO = import.meta.env.DEV
		? "http://localhost:5173/demo/impactite.dev"
		: "https://demo.impactite.dev/demo/impactite.dev";

	// impactite-benchmarks, results/2026-10-15T130903Z.jsonl. impactite runs on its own api,
	// which is plausible's, the others on theirs
	const STATS: {
		label: string;
		unit: string;
		better: "more" | "less";
		digits?: number;
		values: [number, number, number];
	}[] = [
		{ label: "pageviews per second", unit: "/s", better: "more", values: [159799, 7753, 9738] },
		{
			label: "top pages query",
			unit: "ms",
			better: "less",
			digits: 2,
			values: [0.34, 56.5, 1.34]
		},
		{
			label: "cpu for 1M pageviews",
			unit: "s",
			better: "less",
			digits: 1,
			values: [21.5, 1248.2, 137.9]
		},
		{ label: "idle memory", unit: "MiB", better: "less", values: [18, 942, 37] },
		{ label: "idle cpu", unit: "%", better: "less", digits: 2, values: [0, 2.69, 0.02] },
		{ label: "memory after 1M", unit: "MiB", better: "less", values: [124, 2217, 482] },
		{ label: "image size", unit: "MiB", better: "less", values: [19, 987, 36] },
		{ label: "tracker gzipped", unit: "B", better: "less", values: [930, 1324, 3259] }
	];
	const TOOLS = ["impactite", "plausible", "goatcounter"];

	// 28 days of the demo site, with the day it hit the front page
	const days = Array.from({ length: 28 }, (_, i) => {
		const weekly = [0.82, 1, 1.04, 1.02, 0.97, 0.7, 0.62][i % 7];
		const spike = i === 18 ? 4.3 : i === 19 ? 1.9 : 1;
		return Math.round((210 + i * 6) * weekly * spike);
	});
	const labels = days.map((_, i) => {
		const date = new Date(Date.UTC(2026, 8, 18 + i));
		return date.toLocaleDateString("en-US", {
			month: "short",
			day: "numeric",
			timeZone: "UTC"
		});
	});
	const compact = (value: number) => Math.round(value).toLocaleString("en-US");
	// the preview wakes up when the meteor lands
	let landed = $state(false);
	// the same six numbers, panels and tabs as the dashboard, filled with the demo site's month
	const TOTALS = [9148, 10512, 21400, 54, 127, 2.04];
	const PREVIOUS = [6629, 7729, 15620, 54, 130, 2.02];
	let metric = $state(0);
	// visits and pageviews follow the visitors line, the rates wander around their average
	const FACTORS = [1, 1.15, 2.34];
	const shape = (i: number) =>
		i < FACTORS.length
			? days.map((day) => Math.round(day * FACTORS[i]))
			: days.map((_, k) => TOTALS[i] * (0.9 + 0.2 * Math.sin(k * 0.7 + i)));
	const PANELS = {
		left: {
			tabs: [
				{ key: "visit:source", label: "sources" },
				{ key: "visit:channel", label: "channels" },
				{ key: "visit:utm_source", label: "utm" }
			],
			rows: {
				"visit:source": [
					["Google", 3811],
					["Direct / None", 2506],
					["Hacker News", 586],
					["Twitter", 521],
					["Reddit", 517]
				],
				"visit:channel": [
					["Organic Search", 4180],
					["Direct", 2506],
					["Organic Social", 1038],
					["Referral", 1004],
					["Email", 420]
				],
				"visit:utm_source": [
					["newsletter", 420],
					["github", 312],
					["mastodon", 118]
				]
			} as Record<string, [string, number][]>
		},
		right: {
			tabs: [
				{ key: "event:page", label: "pages" },
				{ key: "visit:entry_page", label: "entry" },
				{ key: "visit:exit_page", label: "exit" },
				{ key: "event:hostname", label: "hostnames" }
			],
			rows: {
				"event:page": [
					["/", 4377],
					["/pricing", 2185],
					["/blog/crater", 2165],
					["/docs", 1524],
					["/docs/install", 1360]
				],
				"visit:entry_page": [
					["/", 5021],
					["/blog/crater", 1840],
					["/docs/install", 902],
					["/pricing", 611],
					["/docs", 400]
				],
				"visit:exit_page": [
					["/docs/install", 2210],
					["/", 2012],
					["/pricing", 1630],
					["/blog/crater", 1402],
					["/docs", 980]
				],
				"event:hostname": [
					["impactite.dev", 8890],
					["docs.impactite.dev", 1622]
				]
			} as Record<string, [string, number][]>
		}
	};
	let tabs = $state({ left: "visit:source", right: "event:page" });
	const rowsOf = (side: "left" | "right") =>
		PANELS[side].rows[tabs[side]].map(([label, value]) => ({
			key: label,
			label,
			value,
			...(ICONIC.has(tabs[side]) && {
				icon: icon(tabs[side], label),
				fallback: LETTERED.has(tabs[side]) ? label : undefined
			})
		}));

	const FEATURES = [
		{ icon: target, label: "goals and custom events" },
		{ icon: filter, label: "funnels" },
		{ icon: coins, label: "revenue tracking" },
		{ icon: search, label: "google search keywords" },
		{ icon: link, label: "shareable dashboards" },
		{ icon: mail, label: "email and slack reports" },
		{ icon: bell, label: "traffic alerts" },
		{ icon: grid, label: "all sites in one view" },
		{ icon: upload, label: "import from ga4 and umami" },
		{ icon: download, label: "full data export" },
		{ icon: cloud, label: "daily exports to s3" },
		{ icon: shield, label: "bot and ip filtering" },
		{ icon: globe, label: "no cookies" },
		{ icon: key, label: "passkeys and two factor" },
		{ icon: lock, label: "single sign-on" },
		{ icon: users, label: "teams and guest access" }
	];

	// split so the svelte parser does not end the script block
	const end = "</" + "script>";
	const HOST = "stats.example.com";
	const PLAUSIBLE = [
		`<script defer data-domain="example.com" src="https://`,
		{ from: "plausible.io", to: HOST },
		`/js/script.js">${end}`
	];
	const GOATCOUNTER = [
		`<script data-goatcounter="https://`,
		{ from: "example.goatcounter.com", to: HOST },
		`/count" async src="`,
		{ from: "//gc.zgo.at", to: `https://${HOST}` },
		`/count.js">${end}`
	];

	let swap = $state(false);
</script>

<svelte:head>
	<link rel="prefetch" href={DEMO} />
</svelte:head>

<div id="top" class="mx-auto flex max-w-6xl flex-col px-6" use:appear>
	<header class="flex h-16 items-center justify-between">
		<a href="#top" class="flex items-center gap-2 font-mono text-[15px]">
			<img src="/favicon.svg" alt="" width="22" height="22" />
			impactite
		</a>
		<nav class="flex items-center gap-6">
			<a href="#speed" class="hidden label hover:text-fg sm:block">speed</a>
			<a href="#compatible" class="hidden label hover:text-fg sm:block">compatible</a>
			<a href={DEMO} class="label hover:text-fg">demo</a>
			<a href="#install" class="hidden label hover:text-fg sm:block">install</a>
			<a
				href={REPO}
				class="flex items-center gap-1.5 label hover:text-fg"
				aria-label="source on github{data.stars === null ? '' : `, ${data.stars} stars`}"
			>
				<Icon svg={github} size={14} />
				{#if data.stars !== null}
					<Icon svg={star} size={12} />
					<span class="num">{compact(data.stars)}</span>
				{/if}
			</a>
		</nav>
	</header>

	<div class="relative" use:impact={() => (landed = true)}>
		<section class="grid items-center gap-12 py-20 md:grid-cols-[1fr_auto]">
			<div class="shown flex flex-col gap-6" data-appear>
				<h1 class="font-mono text-[44px] leading-[1.05] tracking-tight md:text-[64px]">
					Analytics<br />that land.
				</h1>
				<p class="max-w-md text-[17px] text-dim">
					Self-hosted web analytics in one binary. Drop-in for Plausible and GoatCounter.
				</p>
				<div class="flex gap-3">
					<a
						href="#install"
						class="flex h-9 items-center rounded-px bg-cyan px-4 label text-ink hover:bg-fg"
						>install</a
					>
					<a
						href={DEMO}
						class="flex h-9 items-center rounded-px border border-edge px-4 label text-fg hover:border-dim"
						>live demo</a
					>
				</div>
			</div>
			<div class="relative z-20 justify-self-center" data-rock style="opacity: 0">
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
			<div
				class="px-4 pt-3 pb-5 transition-[clip-path] duration-1000 ease-out"
				style:clip-path={landed ? "inset(0)" : "inset(0 100% 0 0)"}
			>
				<Chart
					values={shape(metric)}
					{labels}
					format={(value) => format(METRICS[metric].key, value)}
					height={200}
				/>
			</div>
		</section>
		<div class="mt-6 grid gap-6 md:grid-cols-2">
			{#each ["left", "right"] as const as side (side)}
				<Panel tabs={PANELS[side].tabs} bind:tab={tabs[side]}>
					<Rows
						rows={rowsOf(side).map((row) => (landed ? row : { ...row, value: 0 }))}
						format={compact}
						limit={5}
					/>
				</Panel>
			{/each}
		</div>
	</div>

	<section id="speed" class="flex flex-col gap-14 py-28">
		<div class="flex flex-col gap-4" data-appear>
			<h2 class="font-mono text-[32px] tracking-tight">Benchmarks</h2>
			<p class="max-w-2xl text-[17px] text-dim">1M pageviews each, one machine.</p>
		</div>
		<div class="grid gap-x-12 gap-y-10 md:grid-cols-2 lg:grid-cols-3">
			{#each STATS as stat, i (stat.label)}
				<div class="flex flex-col gap-4" data-appear style:--delay="{(i % 3) * 80}ms">
					<span class="label">{stat.label}</span>
					<Race
						unit={stat.unit}
						digits={stat.digits ?? 0}
						better={stat.better}
						rows={TOOLS.map((label, i) => ({
							label,
							value: stat.values[i],
							ours: i === 0
						}))}
					/>
				</div>
			{/each}
		</div>
		<p class="text-[14px] text-dim">
			Plausible CE 3.2.1, GoatCounter 2.7.0, Ryzen 5 7600X.
			<a href={BENCH} class="text-cyan hover:text-fg">Source</a>
		</p>
	</section>

	<section id="scale" class="grid gap-12 pb-28 md:grid-cols-[1fr_1.4fr]">
		<div class="flex flex-col gap-4" data-appear>
			<h2 class="font-mono text-[32px] tracking-tight">Do you really need Kubernetes?</h2>
			<p class="text-dim">Set your monthly pageviews to see the load on one server.</p>
		</div>
		<Scale
			cpu={STATS.find((stat) => stat.label === "cpu for 1M pageviews")?.values[0] ?? 0}
			rate={STATS.find((stat) => stat.label === "pageviews per second")?.values[0] ?? 0}
		/>
	</section>

	<section
		id="compatible"
		class="grid gap-12 pb-28 md:grid-cols-[1fr_1.4fr]"
		use:reveal={(on) => (swap = on)}
	>
		<div class="flex flex-col gap-4" data-appear>
			<h2 class="font-mono text-[32px] tracking-tight">
				Migrating from Plausible or GoatCounter?
			</h2>
			<p class="text-dim">
				Just change the host in your script. Your old snippets, custom events and API calls
				keep working.
			</p>
		</div>
		<div class="flex flex-col gap-4">
			<span class="label">plausible</span>
			<Edit parts={PLAUSIBLE} on={swap} delay={300} />
			<span class="label">goatcounter</span>
			<Edit parts={GOATCOUNTER} on={swap} delay={1900} />
		</div>
	</section>

	<section
		class="grid gap-px overflow-hidden rounded-px border border-line bg-line sm:grid-cols-2 lg:grid-cols-4"
	>
		{#each FEATURES as feature, i (feature.label)}
			<div
				class="flex items-center gap-3 bg-panel px-4 py-5"
				data-appear
				style:--delay="{i * 40}ms"
			>
				<Icon svg={feature.icon} size={14} class="text-cyan" />
				<span class="text-[14px]">{feature.label}</span>
			</div>
		{/each}
	</section>

	<section id="install" class="grid gap-12 py-28 md:grid-cols-[1fr_1.4fr]">
		<div class="flex flex-col gap-4" data-appear>
			<h2 class="font-mono text-[32px] tracking-tight">One binary. Done.</h2>
			<p class="text-dim">Pick how you run it and copy the result.</p>
		</div>
		<Install repo={REPO} />
	</section>

	<footer
		class="flex flex-col gap-4 border-t border-line py-8 sm:flex-row sm:items-center sm:justify-between"
	>
		<span class="flex items-center gap-2 font-mono text-[13px] text-dim">
			<img src="/favicon.svg" alt="" width="16" height="16" />
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
