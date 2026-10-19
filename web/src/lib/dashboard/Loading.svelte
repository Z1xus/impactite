<script lang="ts">
	import Kpis from "#lib/ui/Kpis.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";

	let { bare = false }: { bare?: boolean } = $props();
</script>

<div class="flex flex-col gap-6">
	{#if !bare}
		<div class="flex flex-wrap items-center gap-3">
			<Skeleton class="h-control w-44" />
		</div>
	{/if}
	<section class="rounded-px border border-line bg-panel">
		<Kpis totals={null} previous={null} active={0} pick={() => {}} />
		<div class="px-4 pt-3 pb-5">
			<Skeleton class="h-[377px] w-full" />
		</div>
	</section>
	{#if bare}
		<div class="grid grid-cols-1 gap-6 md:grid-cols-2">
			{#each { length: 4 }, i (i)}
				<div class="h-82 rounded-px border border-line bg-panel"></div>
			{/each}
		</div>
	{/if}
</div>
