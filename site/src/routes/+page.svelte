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
	import Meteor from "#lib/art/Meteor.svelte";
	import Chart from "#lib/dither/Chart.svelte";
	import Code from "#lib/ui/Code.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Rows from "#lib/ui/Rows.svelte";
	import Edit from "../lib/Edit.svelte";
	import Race from "../lib/Race.svelte";
	import { appear } from "../lib/appear.ts";
	import { reveal } from "../lib/reveal.ts";

	const REPO = "https://github.com/z1xus/impactite";
	const BENCH = "https://github.com/z1xus/impactite-benchmarks";

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
		{
			label: "disk after 1M",
			unit: "MiB",
			better: "less",
			values: [23.5, 1331.7, 11.6]
		},
		{ label: "image size", unit: "MiB", better: "less", values: [19, 987, 36] },
		{ label: "tracker gzipped", unit: "B", better: "less", values: [930, 1324, 3259] }
	];
	const TOOLS = ["impactite", "plausible", "goatcounter"];
	// the same goatcounter requests and queries sent to both
	const DROPIN = [
		["pageviews per second", "66,488", "9,738"],
		["top pages query", "0.86 ms", "1.34 ms"],
		["cpu for 1M pageviews", "43.9 s", "137.9 s"],
		["memory after 1M", "100 MiB", "482 MiB"],
		["visits counted", "869,192", "869,192"]
	];

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
		{ icon: mail, label: "reports and traffic alerts" },
		{ icon: search, label: "search console terms" },
		{ icon: shield, label: "shields and data controls" },
		{ icon: upload, label: "imports from both" },
		{ icon: download, label: "exports to both" },
		{ icon: globe, label: "no cookies, no fingerprints" },
		{ icon: chartIcon, label: "visitor counter" }
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

	let speed = $state(false);
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
			<a href="#install" class="label hover:text-fg">install</a>
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
					href="#speed"
					class="flex h-9 items-center rounded-px border border-edge px-4 label text-fg hover:border-dim"
					>see it fly</a
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

	<section id="speed" class="flex flex-col gap-14 py-28" use:reveal={(on) => (speed = on)}>
		<div class="flex flex-col gap-4" use:appear>
			<h2 class="font-mono text-[32px] tracking-tight">Measured, side by side.</h2>
			<p class="max-w-2xl text-[17px] text-dim">
				A million pageviews sent to each tool through its own tracker endpoint, then the
				same queries through its own API. One machine, one tool at a time. The best of each
				is solid.
			</p>
		</div>
		<div class="grid gap-x-12 gap-y-10 md:grid-cols-2 lg:grid-cols-3">
			{#each STATS as stat, i (stat.label)}
				<div class="flex flex-col gap-4" use:appear={{ delay: (i % 3) * 80 }}>
					<span class="label">{stat.label}, {stat.better} is better</span>
					<Race
						on={speed}
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
		<div class="flex flex-col gap-4">
			<h3 class="label text-fg">as a goatcounter drop-in</h3>
			<p class="max-w-2xl text-[15px] text-dim">
				The same GoatCounter requests and queries, sent to impactite and to GoatCounter.
				Both count every visit the same.
			</p>
			<div class="max-w-2xl overflow-x-auto rounded-px border border-line">
				<table class="w-full min-w-[22rem] text-[14px]">
					<thead>
						<tr class="border-b border-line">
							<th class="px-4 py-2 text-left label font-normal"></th>
							<th class="px-4 py-2 text-right label font-normal text-fg">impactite</th
							>
							<th class="px-4 py-2 text-right label font-normal">goatcounter</th>
						</tr>
					</thead>
					<tbody>
						{#each DROPIN as [label, ours, theirs] (label)}
							<tr class="border-b border-line last:border-0">
								<td class="px-4 py-2 text-dim">{label}</td>
								<td class="px-4 py-2 text-right font-mono">{ours}</td>
								<td class="px-4 py-2 text-right font-mono text-dim">{theirs}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</div>
		<p class="text-[14px] text-dim">
			Ryzen 5 7600X, Podman, October 15, 2026. Plausible CE 3.2.1, GoatCounter 2.7.0. Disk
			holds an open day, GoatCounter keeps only counts while impactite keeps every event, so
			any filter works later.
			<a href={BENCH} class="text-cyan hover:text-fg">See the benchmark sources</a> and run it yourself.
		</p>
	</section>

	<section
		id="compatible"
		class="grid gap-12 pb-28 md:grid-cols-[1fr_1.4fr]"
		use:reveal={(on) => (swap = on)}
	>
		<div class="flex flex-col gap-4" use:appear>
			<h2 class="font-mono text-[32px] tracking-tight">Just change the host.</h2>
			<p class="text-dim">
				Fully compatible with the Plausible and GoatCounter trackers. Their APIs and exports
				work as they are, and your history comes along.
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
			<p class="text-dim">Static builds for Linux, or a scratch Docker image.</p>
		</div>
		<div class="flex flex-col gap-4">
			<span class="label">binary</span>
			<Code
				code={`curl -L ${REPO}/releases/latest/download/impactite-x86_64-unknown-linux-musl.tar.xz | tar xJ\n./impactite serve --url https://stats.example.com --acme`}
			/>
			<span class="label">docker</span>
			<Code
				code="docker run -d -p 443:8000 -p 80:80 -v impactite:/data ghcr.io/z1xus/impactite serve --url https://stats.example.com --acme"
			/>
		</div>
	</section>

	<footer class="flex items-center justify-between border-t border-line py-8">
		<span class="flex items-center gap-2 font-mono text-[13px] text-dim">
			<img src="/favicon.svg" alt="" width="16" height="16" />
			impactite
		</span>
		<span class="label">agpl 3.0, tracker mit</span>
	</footer>
</div>
