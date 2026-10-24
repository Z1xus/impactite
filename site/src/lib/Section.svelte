<script lang="ts">
	import type { Snippet } from "svelte";

	let {
		id,
		title,
		note,
		art,
		flip = false,
		children
	}: {
		id?: string;
		title: string;
		note?: string;
		art: Snippet;
		flip?: boolean;
		children: Snippet;
	} = $props();
</script>

<section
	{id}
	class="grid items-center gap-x-16 gap-y-10 {flip
		? 'md:grid-cols-[1.5fr_1fr]'
		: 'md:grid-cols-[1fr_1.5fr]'}"
>
	<div
		class="grid place-items-center {flip ? 'md:order-2' : ''}"
		data-appear
		style:--delay="160ms"
	>
		{@render art()}
	</div>
	<div class="flex min-w-0 flex-col gap-8">
		<div class="flex flex-col gap-4" data-appear>
			<h2 class="font-mono text-heading tracking-tight extrude">{title}</h2>
			{#if note}<p class="max-w-xl text-dim">{note}</p>{/if}
		</div>
		<div data-appear style:--delay="80ms">{@render children()}</div>
	</div>
</section>
