<script lang="ts">
	import type { Snippet } from "svelte";

	let {
		id,
		title,
		note,
		children
	}: { id?: string; title: string; note?: string; children: Snippet } = $props();
</script>

<section {id} class="grid gap-12 pb-20 md:grid-cols-[1fr_1.4fr] md:pb-28">
	<div class="flex flex-col gap-4" data-appear>
		<h2 class="font-mono text-heading tracking-tight">{title}</h2>
		{#if note}<p class="text-dim">{note}</p>{/if}
	</div>
	<div data-appear style:--delay="80ms">{@render children()}</div>
</section>
