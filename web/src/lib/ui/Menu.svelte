<script lang="ts">
	import { preloadData } from "$app/navigation";
	import { sentence } from "./text";
	import chevron from "pixelarticons/svg/chevron-down.svg?raw";
	import check from "pixelarticons/svg/check.svg?raw";
	import { fit } from "./fit.ts";
	import Avatar from "./Avatar.svelte";
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
		avatar,
		items,
		align = "left",
		data = false,
		plain = false,
		iconOnly = false
	}: {
		label: string;
		icon?: string;
		avatar?: number;
		items: Item[];
		align?: "left" | "right";
		data?: boolean;
		plain?: boolean;
		iconOnly?: boolean;
	} = $props();

	let open = $state(false);
	let root: HTMLDivElement;

	$effect(() => {
		if (!open) return;
		for (const item of items) {
			if (item !== "divider" && item.href) void preloadData(item.href).catch(() => {});
		}
	});

	function outside(event: PointerEvent) {
		if (open && !root.contains(event.target as Node)) open = false;
	}
</script>

<svelte:window
	onpointerdown={outside}
	onkeydown={(event) => event.key === "Escape" && (open = false)}
/>

<div class="relative min-w-0" bind:this={root}>
	<button
		type="button"
		class="flex h-9 cursor-pointer items-center gap-2 rounded-px px-3 whitespace-nowrap text-fg transition-colors duration-100 {plain
			? 'font-sans text-[14px] hover:bg-raised'
			: 'border border-edge font-sans text-[14px] hover:border-dim'} {data
			? 'tracking-normal normal-case'
			: ''}"
		aria-expanded={open}
		aria-label={iconOnly ? label : undefined}
		onclick={() => (open = !open)}
	>
		{#if avatar !== undefined}
			<Avatar seed={avatar} size={18} />
		{:else if icon}
			<Icon svg={icon} size={12} class="text-dim" />
		{/if}
		{#if !iconOnly}
			<span class="max-w-32 truncate sm:max-w-56">{data ? label : sentence(label)}</span>
			<Icon svg={chevron} size={12} class="text-dim" />
		{/if}
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
							class="flex h-9 w-full cursor-pointer items-center gap-2.5 px-3 text-left font-sans text-[14px] whitespace-nowrap transition-colors duration-75 hover:bg-line {item.current
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
							<span class="flex-1">{data ? item.label : sentence(item.label)}</span>
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
