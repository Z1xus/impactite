<script lang="ts">
	import { glyph } from "#lib/uptime/glyph.ts";
	import { latency, percent } from "#lib/uptime/format.ts";
	import type { Monitor } from "#lib/uptime/types.ts";
	import Glyph from "./Glyph.svelte";
	import Skeleton from "./Skeleton.svelte";
	import State from "./State.svelte";
	import Strip from "./Strip.svelte";

	let {
		monitor,
		today = 0,
		href,
		warm,
		head = false
	}: {
		monitor?: Monitor;
		today?: number;
		href?: string;
		warm?: () => void;
		head?: boolean;
	} = $props();

	const columns =
		"grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-4 gap-y-3 px-5 md:grid-cols-[minmax(10rem,1fr)_7rem_minmax(0,540px)_5rem_5rem]";
</script>

{#if head}
	<div class="{columns} py-3 max-md:hidden">
		<span class="label">monitor</span>
		<span class="label">status</span>
		<span class="label">90 days</span>
		<span class="text-right label">uptime</span>
		<span class="text-right label">response</span>
	</div>
{:else if monitor}
	<!-- eslint-disable svelte/no-navigation-without-resolve -- callers pass resolved paths -->
	<a {href} onpointerenter={warm} onfocus={warm} class="{columns} py-4 hover:bg-raised">
		<span class="flex min-w-0 items-center gap-3">
			<Glyph spec={glyph(monitor)} fallback={monitor.name} />
			<span class="flex min-w-0 flex-col gap-1.5">
				<span class="truncate text-body text-fg">{monitor.name}</span>
				<span class="truncate meta leading-4"
					>{monitor.kind === "heartbeat" ? "cron job" : monitor.target}</span
				>
			</span>
		</span>
		<span class="md:order-none"><State state={monitor.state} since={monitor.since} /></span>
		<span class="col-span-2 flex items-center gap-4 md:contents">
			<span class="flex min-w-0 flex-1 justify-end md:flex-none">
				<Strip cells={monitor.strip} {today} paused={monitor.paused} />
			</span>
			<span class="w-[4.5rem] text-right num text-small text-fg md:w-auto">
				{monitor.uptime_30d === null ? "–" : percent(monitor.uptime_30d)}
			</span>
			<span class="hidden text-right num text-small text-fg md:block">
				{monitor.p50_24h === null ? "–" : latency(monitor.p50_24h)}
			</span>
		</span>
	</a>
	<!-- eslint-enable svelte/no-navigation-without-resolve -->
{:else}
	<div class="{columns} pointer-events-none py-4">
		<span class="flex items-center gap-3">
			<Skeleton class="size-5" />
			<Skeleton class="h-4 w-36" />
		</span>
		<Skeleton class="h-3 w-16" />
		<span class="col-span-2 md:contents">
			<Skeleton class="h-[22px] w-full max-w-[540px]" />
			<Skeleton class="hidden h-3 w-12 justify-self-end md:block" />
			<Skeleton class="hidden h-3 w-12 justify-self-end md:block" />
		</span>
	</div>
{/if}
