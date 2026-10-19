<script lang="ts">
	import type { Snippet } from "svelte";

	let {
		id,
		title,
		note,
		children
	}: { id?: string; title: string; note?: string; children: Snippet } = $props();
</script>

<section {id} class="grid gap-12 md:grid-cols-[1fr_1.4fr]">
	<div class="flex flex-col gap-4 self-start md:sticky md:top-8" data-appear>
		<h2 class="font-mono text-heading tracking-tight">{title}</h2>
		{#if note}<p class="text-dim">{note}</p>{/if}
	</div>
	<div class="min-w-0" data-appear style:--delay="80ms">{@render children()}</div>
</section>
