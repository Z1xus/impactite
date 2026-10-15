<script lang="ts">
	import type { Snippet } from "svelte";
	import grip from "pixelarticons/svg/drag-and-drop.svg?raw";
	import close from "pixelarticons/svg/close.svg?raw";
	import Icon from "./Icon.svelte";

	let {
		tabs,
		tab = $bindable(),
		actions,
		footer,
		arranging = false,
		hide,
		children
	}: {
		tabs: { key: string; label: string }[];
		tab: string;
		actions?: Snippet;
		footer?: Snippet;
		// layout editing shows a grip and a close button, the whole panel drags
		arranging?: boolean;
		hide?: () => void;
		children: Snippet;
	} = $props();
</script>

<section
	class="flex h-full min-w-0 flex-col rounded-px border bg-panel {arranging
		? 'cursor-grab border-dashed border-dim active:cursor-grabbing'
		: 'border-line'}"
>
	<header class="flex h-11 items-center gap-4 border-b border-line px-4">
		{#if arranging}
			<span class="-mr-2 -ml-2 grid h-7 w-5 place-items-center text-dim" aria-hidden="true">
				<Icon svg={grip} size={12} />
			</span>
		{/if}
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
		<div class="ml-auto flex items-center gap-2">
			{#if actions}
				{@render actions()}
			{/if}
			{#if arranging && hide}
				<button
					type="button"
					aria-label="hide panel"
					title="hide panel"
					class="-mr-2 grid h-7 w-7 cursor-pointer place-items-center text-dim hover:text-fg"
					onclick={hide}
				>
					<Icon svg={close} size={12} />
				</button>
			{/if}
		</div>
	</header>
	<div class="flex-1 p-2 {arranging ? 'pointer-events-none opacity-50 select-none' : ''}">
		{@render children()}
	</div>
	{#if footer && !arranging}
		<footer class="flex justify-end border-t border-line px-2 py-1">
			{@render footer()}
		</footer>
	{/if}
</section>
