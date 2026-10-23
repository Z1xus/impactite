<script lang="ts">
	import type { Spec } from "#lib/icons/resolve.ts";
	import Glyph from "./Glyph.svelte";

	let {
		spec,
		swatch,
		label,
		on = false,
		dim = false,
		title,
		onclick
	}: {
		spec?: Spec;
		swatch?: string;
		label: string;
		on?: boolean;
		dim?: boolean;
		title?: string;
		onclick: () => void;
	} = $props();
</script>

<button
	type="button"
	class="inline-flex h-7 items-center gap-1.5 rounded-px border pr-2.5 pl-1 text-small transition-[color,border-color,opacity] duration-100 {on
		? 'border-cyan text-fg'
		: 'border-line text-dim hover:border-edge hover:text-fg'} {dim && !on ? 'opacity-40' : ''}"
	aria-pressed={on}
	{title}
	{onclick}
>
	{#if spec}
		<Glyph {spec} />
	{:else}
		<span class="m-1 size-3 rounded-[2px]" style:background={swatch}></span>
	{/if}
	{label}
</button>
