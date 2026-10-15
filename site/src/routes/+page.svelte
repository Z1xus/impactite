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
	import Count from "../lib/Count.svelte";
	import Race from "../lib/Race.svelte";
	import { reveal } from "../lib/reveal.ts";

	const REPO = "https://github.com/z1xus/impactite";

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
	const PLAUSIBLE = `<script defer data-domain="example.com" src="https://stats.example.com/js/script.js">${end}`;
	const GOATCOUNTER = `<script data-goatcounter="https://stats.example.com/count" async src="https://stats.example.com/count.js">${end}`;

	let speed = $state(false);
	let size = $state(false);
	let stack = $state(false);
</script>

<div id="top" class="mx-auto flex max-w-6xl flex-col px-6">
	<header class="flex h-16 items-center justify-between">
		<a href="#top" class="flex items-center gap-2 font-mono text-[15px]">
			<img src="/favicon.svg" alt="" width="22" height="22" />
			impactite
		</a>
		<nav class="flex items-center gap-6">
			<a href="#speed" class="label hover:text-fg">speed</a>
			<a href="#compatible" class="label hover:text-fg">compatible</a>
			<a href="#install" class="label hover:text-fg">install</a>
			<a href={REPO} class="label hover:text-fg">source</a>
		</nav>
	</header>

	<section class="grid items-center gap-12 py-20 md:grid-cols-[1fr_auto]">
		<div class="flex flex-col gap-6">
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
		<div class="justify-self-center"><Meteor size={340} pixel={4} /></div>
	</section>

	<section class="rounded-px border border-line bg-panel">
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

	<section id="speed" class="flex flex-col gap-10 py-28" use:reveal={(on) => (speed = on)}>
		<h2 class="font-mono text-[32px] tracking-tight">Built on its own engine.</h2>
		<div
			class="grid gap-px overflow-hidden rounded-px border border-line bg-line md:grid-cols-4"
		>
			{#each [{ value: 2, digits: 0, unit: "µs", label: "warm query, 9M events" }, { value: 3, digits: 0, unit: "ms", label: "cold query, same data" }, { value: 1.1, digits: 1, unit: "M/s", label: "events ingested" }, { value: 12, digits: 0, unit: "B", label: "stored per event" }] as stat, i (stat.label)}
				<div class="flex flex-col gap-3 bg-panel p-6">
					<span class="text-big">
						<Count value={stat.value} on={speed} digits={stat.digits} delay={i * 220} />
						<span class="font-mono text-[18px] text-cyan">{stat.unit}</span>
					</span>
					<span class="label">{stat.label}</span>
				</div>
			{/each}
		</div>
	</section>

	<section class="grid gap-16 pb-28 md:grid-cols-2">
		<div class="flex flex-col gap-6" use:reveal={(on) => (size = on)}>
			<h3 class="label">tracker, brotli</h3>
			<Race
				on={size}
				unit="B"
				rows={[
					{ label: "impactite", value: 795, ours: true },
					{ label: "plausible", value: 1123 },
					{ label: "goatcounter", value: 2795 }
				]}
			/>
		</div>
		<div class="flex flex-col gap-6" use:reveal={(on) => (stack = on)}>
			<h3 class="label">services to run</h3>
			<Race
				on={stack}
				unit=""
				rows={[
					{ label: "impactite", value: 1, ours: true },
					{ label: "goatcounter", value: 1 },
					{ label: "plausible", value: 3 }
				]}
			/>
		</div>
	</section>

	<section id="compatible" class="grid gap-12 pb-28 md:grid-cols-[1fr_1.4fr]">
		<div class="flex flex-col gap-4">
			<h2 class="font-mono text-[32px] tracking-tight">Change one host.</h2>
			<p class="text-dim">
				Their scripts, APIs and exports work as they are. Bring your history along.
			</p>
		</div>
		<div class="flex flex-col gap-4">
			<span class="label">plausible</span>
			<Code code={PLAUSIBLE} />
			<span class="label">goatcounter</span>
			<Code code={GOATCOUNTER} />
		</div>
	</section>

	<section
		class="grid grid-cols-2 gap-px overflow-hidden rounded-px border border-line bg-line md:grid-cols-4"
	>
		{#each FEATURES as feature (feature.label)}
			<div class="flex items-center gap-3 bg-panel px-4 py-5">
				<Icon svg={feature.icon} size={14} class="text-cyan" />
				<span class="text-[14px]">{feature.label}</span>
			</div>
		{/each}
	</section>

	<section id="install" class="grid gap-12 py-28 md:grid-cols-[1fr_1.4fr]">
		<div class="flex flex-col gap-4">
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
