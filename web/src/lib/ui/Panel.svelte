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
		warm,
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
		warm?: (key: string) => void;
		actions?: Snippet;
		expand?: () => void;
		// layout editing shows a close button and the whole panel drags
		arranging?: boolean;
		hide?: () => void;
		move?: (step: number) => void;
		fit?: boolean;
		children: Snippet;
	} = $props();

	let strip: HTMLElement | undefined = $state();
	let edges = $state({ start: false, end: false });

	function measure() {
		if (!strip) return;
		edges = {
			start: strip.scrollLeft > 1,
			end: strip.scrollLeft + strip.clientWidth < strip.scrollWidth - 1
		};
	}

	$effect(() => {
		const current = strip?.children[tabs.findIndex((item) => item.key === tab)] as
			HTMLElement | undefined;
		if (strip && current) {
			strip.scrollLeft = current.offsetLeft - (strip.clientWidth - current.offsetWidth) / 2;
		}
		measure();
	});

	const arrange = $derived([
		...(move
			? [
					{ label: "move up", svg: up, act: () => move(-1), touch: true },
					{ label: "move down", svg: down, act: () => move(1), touch: true }
				]
			: []),
		...(hide ? [{ label: "hide panel", svg: close, act: hide, touch: false }] : [])
	]);
</script>

{#snippet control(item: (typeof arrange)[number], extra: string)}
	<button
		type="button"
		aria-label={item.label}
		title={item.label}
		class="size-control place-items-center text-dim hover:text-fg {extra}"
		onclick={item.act}
	>
		<Icon svg={item.svg} size={12} />
	</button>
{/snippet}

<section
	class="flex h-full min-w-0 flex-col rounded-px border bg-panel {arranging
		? 'cursor-grab border-dashed border-dim active:cursor-grabbing'
		: 'border-line'}"
>
	<header class="relative flex h-11 items-center gap-4 border-b border-line px-4">
		<div
			bind:this={strip}
			onscroll={measure}
			style:--fade-start={edges.start ? "16px" : "0px"}
			style:--fade-end={edges.end ? "16px" : "0px"}
			class="-ml-2 flex h-full min-w-0 [scrollbar-width:none] items-center overflow-x-auto fade-x"
		>
			{#each tabs as item (item.key)}
				<button
					type="button"
					class="h-full shrink-0 border-b px-2 font-sans text-small {tab === item.key
						? 'border-cyan text-fg'
						: 'border-transparent hover:text-fg'}"
					aria-pressed={tab === item.key}
					onpointerenter={() => warm?.(item.key)}
					onfocus={() => warm?.(item.key)}
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
			{#if arranging}
				{#each arrange.filter((item) => !item.touch) as item (item.label)}
					{@render control(item, "-mr-2 grid pointer-coarse:hidden")}
				{/each}
			{/if}
		</div>
		<span class="rule"></span>
	</header>
	{#if arranging && arrange.length}
		<div class="hidden justify-end gap-2 border-b border-line px-2 pointer-coarse:flex">
			{#each arrange as item (item.label)}
				{@render control(item, "grid")}
			{/each}
		</div>
	{/if}
	<div
		class="flex-1 px-2 py-3 {fit ? '' : 'min-h-76'} {arranging
			? 'pointer-events-none opacity-50 select-none'
			: ''}"
	>
		{@render children()}
	</div>
</section>
