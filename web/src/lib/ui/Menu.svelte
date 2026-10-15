<script lang="ts">
	import chevron from "pixelarticons/svg/chevron-down.svg?raw";
	import check from "pixelarticons/svg/check.svg?raw";
	import { fit } from "./fit.ts";
	import Icon from "./Icon.svelte";

	type Item =
		| {
				label: string;
				href?: string;
				onclick?: () => void;
				icon?: string;
				current?: boolean;
		  }
		| "divider";

	let {
		label,
		icon,
		items,
		align = "left",
		data = false
	}: {
		label: string;
		icon?: string;
		items: Item[];
		align?: "left" | "right";
		data?: boolean;
	} = $props();

	let open = $state(false);
	let root: HTMLDivElement;

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
		class="flex h-9 cursor-pointer items-center gap-2 rounded-px border border-edge px-3 label whitespace-nowrap text-fg transition-colors duration-100 hover:border-dim {data
			? 'tracking-normal normal-case'
			: ''}"
		aria-expanded={open}
		onclick={() => (open = !open)}
	>
		{#if icon}
			<Icon svg={icon} size={12} class="text-dim" />
		{/if}
		{label}
		<Icon svg={chevron} size={12} class="text-dim" />
	</button>
	{#if open}
		<ul
			role="menu"
			{@attach fit}
			class="absolute top-full {align === 'right'
				? 'right-0'
				: 'left-0'} z-30 mt-1 min-w-52 rounded-px border border-edge bg-raised py-1"
		>
			{#each items as item, i (i)}
				{#if item === "divider"}
					<li role="separator" class="my-1 border-t border-line"></li>
				{:else}
					<li>
						<!-- eslint-disable svelte/no-navigation-without-resolve -- hrefs come resolved from callers -->
						<svelte:element
							this={item.href ? "a" : "button"}
							role="menuitem"
							tabindex="0"
							href={item.href}
							type={item.href ? undefined : "button"}
							class="flex h-9 w-full cursor-pointer items-center gap-2.5 px-3 text-left label whitespace-nowrap transition-colors duration-75 hover:bg-line {item.current
								? 'text-cyan'
								: 'text-fg'}"
							onclick={() => {
								open = false;
								item.onclick?.();
							}}
						>
							{#if item.icon}
								<Icon svg={item.icon} size={12} class="text-dim" />
							{/if}
							<span class="flex-1">{item.label}</span>
							{#if item.current}
								<Icon svg={check} size={12} />
							{/if}
						</svelte:element>
						<!-- eslint-enable svelte/no-navigation-without-resolve -->
					</li>
				{/if}
			{/each}
		</ul>
	{/if}
</div>
