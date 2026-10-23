<script lang="ts">
	import type { Snippet } from "svelte";
	import chevron from "pixelarticons/svg/chevron-down.svg?raw";
	import Icon from "./Icon.svelte";

	let { label, children }: { label: string; children: Snippet } = $props();

	let open = $state(false);
</script>

<div class="flex flex-col gap-4">
	<button
		type="button"
		class="flex h-compact w-fit items-center gap-2 font-sans text-body text-dim hover:text-fg"
		aria-expanded={open}
		onclick={() => (open = !open)}
	>
		{label}
		<Icon svg={chevron} size={12} class={open ? "rotate-180" : ""} />
	</button>
	{#if open}
		{@render children()}
	{/if}
</div>
