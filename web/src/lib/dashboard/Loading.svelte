<script lang="ts">
	import Kpis from "#lib/ui/Kpis.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";
</script>

<div class="flex flex-col gap-6">
	<div class="flex flex-wrap items-center gap-3">
		<Skeleton class="h-9 w-44" />
	</div>
	<section class="rounded-px border border-line bg-panel">
		<Kpis totals={null} previous={null} active={0} pick={() => {}} />
		<div class="px-4 pt-3 pb-5">
			<Skeleton class="h-[279px] w-full" />
		</div>
	</section>
</div>
