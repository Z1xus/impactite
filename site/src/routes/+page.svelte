<script lang="ts">
	import chartIcon from "pixelarticons/svg/chart.svg?raw";
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
	import theme from "pixelarticons/svg/cloud-moon.svg?raw";
	import Meteor from "#lib/art/Meteor.svelte";
	import Chart from "#lib/dither/Chart.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Rows from "#lib/ui/Rows.svelte";
	import Edit from "../lib/Edit.svelte";
	import Install from "../lib/Install.svelte";
	import Race from "../lib/Race.svelte";
	import { appear } from "../lib/appear.ts";
	import { reveal } from "../lib/reveal.ts";

	const REPO = "https://github.com/z1xus/impactite";
	const BENCH = "https://github.com/z1xus/impactite-benchmarks";
	// a public dashboard on a demo instance, served over tailscale until there's a real host
	const DEMO = "https://hq.neko-notothen.ts.net:10002/share/impactite.dev";

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

	const sources = [
		{ key: "google", label: "Google", value: 3811 },
		{ key: "direct", label: "Direct / None", value: 2506 },
		{ key: "hn", label: "Hacker News", value: 586 },
		{ key: "twitter", label: "Twitter", value: 521 },
		{ key: "reddit", label: "Reddit", value: 517 }
	];
	const pages = [
		{ key: "/", label: "/", value: 4377 },
		{ key: "/pricing", label: "/pricing", value: 2185 },
		{ key: "/blog/crater", label: "/blog/crater", value: 2165 },
		{ key: "/docs", label: "/docs", value: 1524 },
		{ key: "/docs/install", label: "/docs/install", value: 1360 }
	];

	const FEATURES = [
		{ icon: key, label: "passkeys and two factor" },
		{ icon: users, label: "teams and guests" },
		{ icon: filter, label: "funnels and segments" },
		{ icon: coins, label: "revenue goals" },
		{ icon: link, label: "shared and embedded dashboards" },
		{ icon: mail, label: "reports to mail, slack or webhooks" },
		{ icon: search, label: "search console terms" },
		{ icon: shield, label: "shields and data controls" },
		{ icon: upload, label: "ga4 and umami imports" },
		{ icon: download, label: "exports to both" },
		{ icon: globe, label: "no cookies, no fingerprints" },
		{ icon: chartIcon, label: "visitor counter" },
		{ icon: lock, label: "single sign-on" },
		{ icon: cloud, label: "daily raw exports to s3" },
		{ icon: grid, label: "all sites in one view" },
		{ icon: theme, label: "dark and light" }
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

<div id="top" class="mx-auto flex max-w-6xl flex-col px-6">
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
			<a href={REPO} class="label hover:text-fg">source</a>
		</nav>
	</header>

	<section class="grid items-center gap-12 py-20 md:grid-cols-[1fr_auto]">
		<div class="flex flex-col gap-6" use:appear>
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
		<div class="justify-self-center" use:appear={{ kind: "wipe", delay: 150 }}>
			<Meteor size={340} pixel={4} />
		</div>
	</section>

	<section class="rounded-px border border-line bg-panel" use:appear={{ kind: "wipe" }}>
		<div class="grid grid-cols-2 border-b border-line md:grid-cols-4">
			{#each [["visitors", "9,148", "+38%"], ["pageviews", "21.4k", "+37%"], ["bounce", "54%", "+0pt"], ["duration", "2m 07s", "-2%"]] as [name, value, change], i (name)}
				<div class="flex flex-col gap-2 p-4 {i ? 'border-l border-line' : ''}">
					<span class="label {i ? '' : 'text-cyan'}">{name}</span>
					<span class="num text-num">{value}</span>
					<span class="label {change.startsWith('-') ? 'text-bad' : 'text-cyan'}"
						>{change}</span
					>
				</div>
			{/each}
		</div>
		<div class="p-4">
			<Chart values={days} {labels} format={compact} height={200} />
		</div>
		<div class="grid border-t border-line md:grid-cols-2">
			<div class="p-2"><Rows rows={sources} format={compact} limit={5} /></div>
			<div class="border-line p-2 md:border-l">
				<Rows rows={pages} format={compact} limit={5} />
			</div>
		</div>
	</section>

	<section id="speed" class="flex flex-col gap-14 py-28">
		<div class="flex flex-col gap-4" use:appear>
			<h2 class="font-mono text-[32px] tracking-tight">Benchmarks</h2>
			<p class="max-w-2xl text-[17px] text-dim">1M pageviews each, one machine.</p>
		</div>
		<div class="grid gap-x-12 gap-y-10 md:grid-cols-2 lg:grid-cols-3">
			{#each STATS as stat, i (stat.label)}
				<div class="flex flex-col gap-4" use:appear={{ delay: (i % 3) * 80 }}>
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

	<section
		id="compatible"
		class="grid gap-12 pb-28 md:grid-cols-[1fr_1.4fr]"
		use:reveal={(on) => (swap = on)}
	>
		<div class="flex flex-col gap-4" use:appear>
			<h2 class="font-mono text-[32px] tracking-tight">Just change the host.</h2>
			<p class="text-dim">Works with your Plausible or GoatCounter script.</p>
		</div>
		<div class="flex flex-col gap-4">
			<span class="label">plausible</span>
			<Edit parts={PLAUSIBLE} on={swap} delay={300} />
			<span class="label">goatcounter</span>
			<Edit parts={GOATCOUNTER} on={swap} delay={1900} />
		</div>
	</section>

	<section
		class="grid grid-cols-2 gap-px overflow-hidden rounded-px border border-line bg-line md:grid-cols-4"
	>
		{#each FEATURES as feature, i (feature.label)}
			<div class="flex items-center gap-3 bg-panel px-4 py-5" use:appear={{ delay: i * 40 }}>
				<Icon svg={feature.icon} size={14} class="text-cyan" />
				<span class="text-[14px]">{feature.label}</span>
			</div>
		{/each}
	</section>

	<section id="install" class="grid gap-12 py-28 md:grid-cols-[1fr_1.4fr]">
		<div class="flex flex-col gap-4" use:appear>
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
