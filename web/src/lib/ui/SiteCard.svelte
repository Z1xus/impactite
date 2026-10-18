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
		values
	}: { href: string; title: string; glyph?: Spec | null; values?: number[] } = $props();
</script>

<!-- eslint-disable svelte/no-navigation-without-resolve -- callers pass resolved paths -->
<a
	{href}
	class="flex flex-col gap-4 rounded-px border border-line bg-panel p-5 transition-colors duration-100 hover:border-edge"
>
	<span class="flex h-5 min-w-0 items-center gap-2.5 text-fg">
		{#if glyph !== undefined}<Glyph spec={glyph} fallback={title} />{/if}
		<span class="truncate">{title}</span>
	</span>
	<span class="flex h-[22px] items-baseline gap-2">
		{#if values}
			<Num
				value={values.reduce((sum, value) => sum + value, 0)}
				format={compact}
				class="text-num text-fg"
			/>
			<span class="label">visitors in 28 days</span>
		{:else}
			<Skeleton class="h-[22px] w-16" />
		{/if}
	</span>
	<Chart values={values ?? []} axes={false} height={56} format={compact} />
</a>
<!-- eslint-enable svelte/no-navigation-without-resolve -->
