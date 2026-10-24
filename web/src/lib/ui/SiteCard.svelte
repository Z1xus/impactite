<script lang="ts">
	import Chart from "#lib/dither/Chart.svelte";
	import type { Spec } from "#lib/icons/resolve.ts";
	import { compact } from "#lib/stats.ts";
	import { percent } from "#lib/uptime/format.ts";
	import type { Monitor } from "#lib/uptime/types.ts";
	import Glyph from "./Glyph.svelte";
	import Num from "./Num.svelte";
	import Skeleton from "./Skeleton.svelte";
	import State from "./State.svelte";

	let {
		href,
		title,
		glyph,
		values,
		monitor,
		warm
	}: {
		href?: string;
		title?: string;
		glyph?: Spec | null;
		values?: number[];
		monitor?: Monitor;
		warm?: () => void;
	} = $props();
</script>

{#snippet card()}
	<span class="flex h-5 min-w-0 items-center gap-2.5 text-fg">
		{#if title === undefined}
			<Skeleton class="h-4 w-32" />
		{:else}
			{#if glyph !== undefined}<Glyph spec={glyph} fallback={title} />{/if}
			<span class="truncate">{title}</span>
			{#if monitor}
				<span
					class="ml-auto flex shrink-0 items-center"
					title={monitor.uptime_30d === null
						? monitor.state
						: `${monitor.state}, ${percent(monitor.uptime_30d)} in 30 days`}
				>
					<State state={monitor.state} since={monitor.since} />
				</span>
			{/if}
		{/if}
	</span>
	{#if values}
		<span class="flex flex-col gap-1.5">
			<Num
				value={values.reduce((sum, value) => sum + value, 0)}
				format={compact}
				class="text-num text-fg"
			/>
			<span class="label">visitors, 28 days</span>
		</span>
		<Chart {values} axes={false} height={96} format={compact} />
	{:else}
		<Skeleton class="h-[39px] w-full" />
		<Skeleton class="h-[96px] w-full" />
	{/if}
{/snippet}

<!-- an anchor without href breaks link preloading -->
{#if href}
	<!-- eslint-disable svelte/no-navigation-without-resolve -- callers pass resolved paths -->
	<a
		{href}
		onpointerenter={warm}
		onfocus={warm}
		class="flex flex-col gap-4 rounded-px border border-line bg-panel p-5 hover:border-edge"
	>
		{@render card()}
	</a>
	<!-- eslint-enable svelte/no-navigation-without-resolve -->
{:else}
	<div class="flex flex-col gap-4 rounded-px border border-line bg-panel p-5">
		{@render card()}
	</div>
{/if}
