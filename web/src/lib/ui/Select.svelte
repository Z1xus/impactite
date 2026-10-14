<script lang="ts" generics="T extends string | number">
	import chevron from "pixelarticons/svg/chevron-down.svg?raw";
	import type { Spec } from "#lib/icons/resolve.ts";
	import Glyph from "./Glyph.svelte";
	import Icon from "./Icon.svelte";

	let {
		value,
		options,
		pick,
		data = false,
		icon,
		text: shown
	}: {
		value: T;
		options: { key: T; label: string; glyph?: Spec | null }[];
		pick: (key: T) => void;
		data?: boolean;
		icon?: string;
		text?: string;
	} = $props();

	const text = $derived(data ? "normal-case tracking-normal" : "");

	let open = $state(false);
	let root: HTMLDivElement;

	const current = $derived(options.find((option) => option.key === value));

	function choose(key: T) {
		open = false;
		pick(key);
	}

	function outside(event: PointerEvent) {
		if (open && !root.contains(event.target as Node)) open = false;
	}
</script>

<svelte:window
	onpointerdown={outside}
	onkeydown={(event) => event.key === "Escape" && (open = false)}
/>

<div class="relative" bind:this={root}>
	<button
		type="button"
		class="flex h-8 cursor-pointer items-center gap-2 rounded-px border border-line px-3 label whitespace-nowrap {text} text-fg transition-colors duration-100 hover:border-edge"
		aria-expanded={open}
		onclick={() => (open = !open)}
	>
		{#if current?.glyph !== undefined}
			<Glyph spec={current.glyph} fallback={current.label} />
		{:else if icon}
			<Icon svg={icon} size={12} class="text-dim" />
		{/if}
		{shown ?? current?.label}
		<Icon svg={chevron} size={12} class="text-dim" />
	</button>
	{#if open}
		<ul
			class="absolute top-full right-0 z-20 mt-1 max-h-80 min-w-full overflow-y-auto rounded-px border border-edge bg-raised py-1"
		>
			{#each options as option (option.key)}
				<li>
					<button
						type="button"
						class="flex w-full cursor-pointer items-center gap-2 px-3 py-2 text-left label {text} whitespace-nowrap transition-colors duration-75 hover:bg-line {option.key ===
						value
							? 'text-cyan'
							: 'text-fg'}"
						onclick={() => choose(option.key)}
					>
						{#if option.glyph !== undefined}
							<Glyph spec={option.glyph} fallback={option.label} />
						{/if}
						{option.label}
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</div>
