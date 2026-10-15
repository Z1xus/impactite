<script lang="ts" generics="T extends string | number">
	import chevron from "pixelarticons/svg/chevron-down.svg?raw";
	import type { Spec } from "#lib/icons/resolve.ts";
	import Glyph from "./Glyph.svelte";
	import { fit } from "./fit.ts";
	import Icon from "./Icon.svelte";

	let {
		value,
		options,
		pick,
		data = false,
		icon,
		text: shown,
		align = "left",
		label
	}: {
		value: T;
		options: { key: T; label: string; glyph?: Spec | null }[];
		pick: (key: T) => void;
		data?: boolean;
		icon?: string;
		text?: string;
		align?: "left" | "right";
		label?: string;
	} = $props();

	const text = $derived(data ? "normal-case tracking-normal" : "");

	let open = $state(false);
	let root: HTMLDivElement;
	let list = $state<HTMLUListElement>();
	let query = $state("");
	let active = $state(0);

	const current = $derived(options.find((option) => option.key === value));
	// "new york" finds "America/New_York"
	const plain = (text: string) => text.toLowerCase().replaceAll("_", " ");

	// long lists get a search box
	const searchable = $derived(options.length > 12);
	const shownOptions = $derived(
		options.filter((option) => plain(option.label).includes(plain(query.trim())))
	);

	function toggle() {
		open = !open;
		query = "";
		active = Math.max(
			0,
			options.findIndex((option) => option.key === value)
		);
	}

	function choose(key: T) {
		open = false;
		pick(key);
	}

	function keys(event: KeyboardEvent) {
		if (!open) {
			if (event.key === "ArrowDown") {
				event.preventDefault();
				toggle();
			}
			return;
		}
		if (event.key === "ArrowDown" || event.key === "ArrowUp") {
			event.preventDefault();
			const step = event.key === "ArrowDown" ? 1 : -1;
			active = (active + step + shownOptions.length) % Math.max(1, shownOptions.length);
			list?.children[active]?.scrollIntoView({ block: "nearest" });
		} else if (event.key === "Enter" && shownOptions[active]) {
			event.preventDefault();
			choose(shownOptions[active].key);
		}
	}

	function outside(event: PointerEvent) {
		if (open && !root.contains(event.target as Node)) open = false;
	}
</script>

<svelte:window
	onpointerdown={outside}
	onkeydown={(event) => event.key === "Escape" && (open = false)}
/>

<div class="relative flex flex-col gap-2" bind:this={root} onkeydown={keys} role="presentation">
	{#if label}
		<span class="label">{label}</span>
	{/if}
	<button
		type="button"
		class="flex h-9 cursor-pointer items-center gap-2 rounded-px border border-edge px-3 label whitespace-nowrap {text} text-fg transition-colors duration-100 hover:border-dim"
		aria-expanded={open}
		onclick={toggle}
	>
		{#if current?.glyph !== undefined}
			<Glyph spec={current.glyph} fallback={current.label} />
		{:else if icon}
			<Icon svg={icon} size={12} class="text-dim" />
		{/if}
		{shown ?? current?.label}
		<Icon svg={chevron} size={12} class="ml-auto text-dim" />
	</button>
	{#if open}
		<div
			{@attach fit}
			class="absolute top-full {align === 'right'
				? 'right-0'
				: 'left-0'} z-20 mt-1 flex max-h-96 min-w-full flex-col rounded-px border border-edge bg-raised"
		>
			{#if searchable}
				<!-- svelte-ignore a11y_autofocus -->
				<input
					class="m-1 h-8 rounded-px border border-edge bg-panel px-2 text-[14px] text-fg outline-none placeholder:text-faint focus:border-cyan"
					placeholder="search"
					autofocus
					bind:value={query}
					oninput={() => (active = 0)}
				/>
			{/if}
			<ul class="overflow-y-auto py-1" bind:this={list}>
				{#each shownOptions as option, i (option.key)}
					<li>
						<button
							type="button"
							class="flex w-full cursor-pointer items-center gap-2 px-3 py-2 text-left label {text} whitespace-nowrap transition-colors duration-75 hover:bg-line {i ===
							active
								? 'bg-line'
								: ''} {option.key === value ? 'text-cyan' : 'text-fg'}"
							onclick={() => choose(option.key)}
							onpointerenter={() => (active = i)}
						>
							{#if option.glyph !== undefined}
								<Glyph spec={option.glyph} fallback={option.label} />
							{/if}
							{option.label}
						</button>
					</li>
				{:else}
					<li class="px-3 py-2 label text-faint">no match</li>
				{/each}
			</ul>
		</div>
	{/if}
</div>
