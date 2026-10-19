<script lang="ts">
	import Chart from "#lib/dither/Chart.svelte";
	import type { Spec } from "#lib/icons/resolve.ts";
	import { compact } from "#lib/stats.ts";
	import Glyph from "./Glyph.svelte";
	import Num from "./Num.svelte";
	import Skeleton from "./Skeleton.svelte";

	let {
		href,
		title,
		glyph,
		values,
		warm
	}: {
		href?: string;
		title?: string;
		glyph?: Spec | null;
		values?: number[];
		warm?: () => void;
	} = $props();
</script>

<!-- eslint-disable svelte/no-navigation-without-resolve -- callers pass resolved paths -->
<a
	{href}
	onpointerenter={warm}
	onfocus={warm}
	class="flex flex-col gap-4 rounded-px border border-line bg-panel p-5 hover:border-edge"
>
	<span class="flex h-5 min-w-0 items-center gap-2.5 text-fg">
		{#if title === undefined}
			<Skeleton class="h-4 w-32" />
		{:else}
			{#if glyph !== undefined}<Glyph spec={glyph} fallback={title} />{/if}
			<span class="truncate">{title}</span>
		{/if}
	</span>
	{#if values}
		<span class="flex items-end justify-between gap-4">
			<span class="flex flex-col gap-1.5">
				<Num
					value={values.reduce((sum, value) => sum + value, 0)}
					format={compact}
					class="text-num text-fg"
				/>
				<span class="label">visitors, 28 days</span>
			</span>
			<span class="flex flex-col items-end gap-1.5">
				<Num value={values.at(-1) ?? 0} format={compact} class="text-body text-fg" />
				<span class="label">today</span>
			</span>
		</span>
		<Chart {values} axes={false} height={96} format={compact} />
	{:else}
		<Skeleton class="h-[39px] w-full" />
		<Skeleton class="h-[96px] w-full" />
	{/if}
</a>
<!-- eslint-enable svelte/no-navigation-without-resolve -->
