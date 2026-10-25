<script lang="ts">
	import { tick } from "svelte";
	import { preloadData } from "$app/navigation";
	import { sentence } from "./text";
	import chevron from "pixelarticons/svg/chevron-down.svg?raw";
	import check from "pixelarticons/svg/check.svg?raw";
	import { dismiss } from "./popover.ts";
	import { fit } from "./fit.ts";
	import Avatar from "./Avatar.svelte";
	import Icon from "./Icon.svelte";

	type Item =
		| {
				label: string;
				href?: string;
				onclick?: () => void;
				warm?: () => void;
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
		avatar?: boolean;
		items: Item[];
		align?: "left" | "right";
		data?: boolean;
		plain?: boolean;
		iconOnly?: boolean;
	} = $props();

	let open = $state(false);
	let root: HTMLDivElement;
	let trigger: HTMLButtonElement;

	$effect(() => {
		if (!open) return;
		for (const item of items) {
			if (item !== "divider" && item.href) void preloadData(item.href).catch(() => {});
		}
	});

	function entries() {
		return [...root.querySelectorAll<HTMLElement>("[role=menuitem]")];
	}

	async function arrows(event: KeyboardEvent) {
		const keys = ["ArrowDown", "ArrowUp", "Home", "End"];
		if (!keys.includes(event.key)) return;
		event.preventDefault();
		const first = event.key === "ArrowDown" || event.key === "Home";
		if (!open) {
			open = true;
			await tick();
			entries()
				.at(first ? 0 : -1)
				?.focus();
			return;
		}
		const list = entries();
		const at = list.indexOf(document.activeElement as HTMLElement);
		const next =
			event.key === "Home"
				? 0
				: event.key === "End"
					? -1
					: (at < 0 && !first ? -1 : at + (first ? 1 : -1)) % list.length;
		list.at(next)?.focus();
	}
</script>

<div
	class="relative min-w-0"
	bind:this={root}
	{@attach dismiss(
		() => open,
		() => (open = false)
	)}
	onkeydown={arrows}
	role="presentation"
>
	<button
		bind:this={trigger}
		type="button"
		class="flex h-control max-w-full min-w-0 items-center gap-2 rounded-px whitespace-nowrap text-fg {iconOnly
			? 'w-control justify-center'
			: 'px-2.5 sm:px-3'} {plain
			? 'font-sans text-body hover:bg-raised'
			: 'border border-edge font-sans text-body hover:border-dim'} {data
			? 'tracking-normal normal-case'
			: ''}"
		aria-haspopup="menu"
		aria-expanded={open}
		aria-label={iconOnly || avatar ? label : undefined}
		onclick={() => (open = !open)}
	>
		{#if avatar}
			<Avatar name={label} size={18} />
		{:else if icon}
			<Icon svg={icon} size={12} class="text-dim" />
		{/if}
		{#if !iconOnly}
			<span class="max-w-32 min-w-0 truncate sm:max-w-56 {avatar ? 'hidden sm:inline' : ''}"
				>{data ? label : sentence(label)}</span
			>
			<Icon svg={chevron} size={12} class="text-dim" />
		{/if}
	</button>
	{#if open}
		<ul
			role="menu"
			{@attach fit}
			class="popover {align === 'right' ? 'right-0' : 'left-0'} min-w-52 py-1"
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
							tabindex="-1"
							href={item.href}
							type={item.href ? undefined : "button"}
							class="flex h-control w-full items-center gap-2.5 px-3 text-left font-sans text-body whitespace-nowrap hover:bg-line {item.current
								? 'text-cyan'
								: 'text-fg'}"
							onpointerenter={item.warm}
							onfocus={item.warm}
							onclick={() => {
								open = false;
								trigger.focus({ preventScroll: true });
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
