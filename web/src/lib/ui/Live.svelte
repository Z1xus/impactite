<script lang="ts">
	import Num from "./Num.svelte";

	let { site }: { site: string } = $props();

	let count = $state<number | null>(null);

	$effect(() => {
		const source = new EventSource(`/api/app/live/${encodeURIComponent(site)}`);
		source.onmessage = (event) => (count = Number(event.data));
		return () => source.close();
	});
</script>

<span class="flex h-8 items-center gap-2 px-1" title="visitors in the last 5 minutes">
	<span class="h-1.5 w-1.5 {count ? 'bg-cyan' : 'bg-faint'}"></span>
	<Num value={count ?? 0} class="text-label text-fg" />
	<span class="label">now</span>
</span>
