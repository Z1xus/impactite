<script lang="ts">
	import type { Snippet } from "svelte";

	let {
		tabs,
		tab = $bindable(),
		children
	}: { tabs: { key: string; label: string }[]; tab: string; children: Snippet } = $props();
</script>

<section class="flex min-w-0 flex-col rounded-px border border-line bg-panel">
	<header class="flex h-11 items-center gap-4 border-b border-line px-4">
		{#each tabs as item (item.key)}
			<button
				type="button"
				class="h-full cursor-pointer border-b label transition-colors duration-100 {tab ===
				item.key
					? 'border-cyan text-fg'
					: 'border-transparent hover:text-fg'}"
				onclick={() => (tab = item.key)}
			>
				{item.label}
			</button>
		{/each}
	</header>
	<div class="p-2">
		{@render children()}
	</div>
</section>
