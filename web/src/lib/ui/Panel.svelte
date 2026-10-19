<script lang="ts">
	import type { Snippet } from "svelte";
	import close from "pixelarticons/svg/close.svg?raw";
	import up from "pixelarticons/svg/chevron-up.svg?raw";
	import down from "pixelarticons/svg/chevron-down.svg?raw";
	import expand from "pixelarticons/svg/scale.svg?raw";
	import Icon from "./Icon.svelte";
	import { sentence } from "./text";

	let {
		tabs,
		tab = $bindable(),
		actions,
		expand: open,
		arranging = false,
		hide,
		move,
		fit = false,
		children
	}: {
		tabs: { key: string; label: string }[];
		tab: string;
		actions?: Snippet;
		expand?: () => void;
		// layout editing shows a close button and the whole panel drags
		arranging?: boolean;
		hide?: () => void;
		move?: (step: number) => void;
		fit?: boolean;
		children: Snippet;
	} = $props();
</script>

<section
	class="flex h-full min-w-0 flex-col rounded-px border bg-panel {arranging
		? 'cursor-grab border-dashed border-dim active:cursor-grabbing'
		: 'border-line'}"
>
	<header class="relative flex h-11 items-center gap-4 border-b border-line px-4">
		<div class="-ml-2 flex h-full min-w-0 [scrollbar-width:none] items-center overflow-x-auto">
			{#each tabs as item (item.key)}
				<button
					type="button"
					class="h-full shrink-0 border-b px-2 font-sans text-small {tab === item.key
						? 'border-cyan text-fg'
						: 'border-transparent hover:text-fg'}"
					aria-pressed={tab === item.key}
					onclick={() => (tab = item.key)}
				>
					{sentence(item.label)}
				</button>
			{/each}
		</div>
		<div class="ml-auto flex shrink-0 items-center gap-2">
			{#if actions}
				{@render actions()}
			{/if}
			{#if open && !arranging}
				<button
					type="button"
					aria-label="expand report"
					title="expand report"
					class="grid size-control place-items-center text-dim hover:text-fg"
					onclick={open}
				>
					<Icon svg={expand} size={16} />
				</button>
			{/if}
			{#if arranging && move}
				{#each [{ step: -1, svg: up, label: "move up" }, { step: 1, svg: down, label: "move down" }] as item (item.label)}
					<button
						type="button"
						aria-label={item.label}
						title={item.label}
						class="hidden size-control place-items-center text-dim hover:text-fg pointer-coarse:grid"
						onclick={() => move(item.step)}
					>
						<Icon svg={item.svg} size={12} />
					</button>
				{/each}
			{/if}
			{#if arranging && hide}
				<button
					type="button"
					aria-label="hide panel"
					title="hide panel"
					class="-mr-2 grid size-control place-items-center text-dim hover:text-fg"
					onclick={hide}
				>
					<Icon svg={close} size={12} />
				</button>
			{/if}
		</div>
		<span class="rule"></span>
	</header>
	<div
		class="flex-1 px-2 py-3 {fit ? '' : 'min-h-76'} {arranging
			? 'pointer-events-none opacity-50 select-none'
			: ''}"
	>
		{@render children()}
	</div>
</section>
