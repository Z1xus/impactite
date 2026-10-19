<script lang="ts" generics="T extends string | number">
	import { sentence } from "./text";
	import chevron from "pixelarticons/svg/chevron-down.svg?raw";
	import type { Spec } from "#lib/icons/resolve.ts";
	import Glyph from "./Glyph.svelte";
	import { dismiss } from "./popover.ts";
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
		label,
		plain = false,
		title = false,
		footer,
		warm
	}: {
		value: T;
		options: { key: T; label: string; glyph?: Spec | null }[];
		pick: (key: T) => unknown;
		data?: boolean;
		icon?: string;
		text?: string;
		align?: "left" | "right";
		label?: string;
		plain?: boolean;
		title?: boolean;
		footer?: { label: string; href: string; icon: string };
		warm?: (key: T) => void;
	} = $props();

	const text = "font-sans text-body";

	let open = $state(false);
	let pending = $state(false);
	let root: HTMLDivElement;
	let trigger: HTMLButtonElement;
	let list = $state<HTMLUListElement>();
	let query = $state("");
	let active = $state(0);

	const current = $derived(options.find((option) => option.key === value));
	// "new york" finds "America/New_York"
	const normalize = (text: string) => text.toLowerCase().replaceAll("_", " ");

	const searchable = $derived(options.length > 12);
	const shownOptions = $derived(
		options.filter((option) => normalize(option.label).includes(normalize(query.trim())))
	);

	function toggle() {
		open = !open;
		query = "";
		active = Math.max(
			0,
			options.findIndex((option) => option.key === value)
		);
	}

	async function choose(key: T) {
		open = false;
		trigger.focus();
		const result = pick(key);
		if (!(result instanceof Promise)) return;
		pending = true;
		try {
			await result;
		} finally {
			pending = false;
			trigger.focus();
		}
	}

	function keys(event: KeyboardEvent) {
		if (!open) {
			if (event.key === "ArrowDown") {
				event.preventDefault();
				toggle();
			}
			return;
		}
		if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
			event.preventDefault();
			const last = Math.max(0, shownOptions.length - 1);
			const step = event.key === "ArrowDown" ? 1 : -1;
			active =
				event.key === "Home"
					? 0
					: event.key === "End"
						? last
						: (active + step + shownOptions.length) % Math.max(1, shownOptions.length);
			list?.children[active]?.scrollIntoView({ block: "nearest" });
		} else if (event.key === "Enter" && shownOptions[active]) {
			event.preventDefault();
			choose(shownOptions[active].key);
		}
	}
</script>

<div
	class="relative flex flex-col gap-2"
	bind:this={root}
	{@attach dismiss(
		() => open,
		() => (open = false)
	)}
	onkeydown={keys}
	role="presentation"
>
	{#if label}
		<span class="label leading-[14px]">{label}</span>
	{/if}
	<button
		bind:this={trigger}
		type="button"
		class="flex h-control items-center gap-2 rounded-px px-3 whitespace-nowrap {plain
			? `font-sans hover:bg-raised hover:text-fg ${title ? 'text-title font-medium text-fg' : 'text-body text-dim'}`
			: `border border-control ${text} text-fg hover:border-dim`}"
		aria-haspopup="listbox"
		aria-expanded={open}
		disabled={pending}
		onclick={toggle}
	>
		{#if current?.glyph !== undefined}
			<Glyph spec={current.glyph} fallback={current.label} eager />
		{:else if icon}
			<Icon svg={icon} size={12} class="text-dim" />
		{/if}
		<span class={plain ? "max-w-48 truncate" : ""}
			>{shown ?? (current ? (data ? current.label : sentence(current.label)) : "")}</span
		>
		<Icon svg={chevron} size={12} class="ml-auto text-dim" />
	</button>
	{#if open}
		<div
			{@attach fit}
			class="popover {align === 'right' ? 'right-0' : 'left-0'} flex min-w-full flex-col"
		>
			{#if searchable}
				<!-- svelte-ignore a11y_autofocus -->
				<input
					class="m-1 h-8 rounded-px border border-control bg-panel px-2 text-body text-fg outline-none placeholder:text-faint focus:border-cyan"
					placeholder="search"
					autofocus
					bind:value={query}
					oninput={() => (active = 0)}
				/>
			{/if}
			<ul class="min-h-0 overflow-y-auto py-1" role="listbox" bind:this={list}>
				{#each shownOptions as option, i (option.key)}
					<li role="presentation">
						<button
							type="button"
							role="option"
							tabindex="-1"
							aria-selected={option.key === value}
							class="flex w-full items-center gap-2 px-3 py-2 text-left pointer-coarse:py-3 {text} whitespace-nowrap hover:bg-line {i ===
							active
								? 'bg-line'
								: ''} {option.key === value ? 'text-cyan' : 'text-fg'}"
							onclick={() => choose(option.key)}
							onpointerenter={() => {
								active = i;
								warm?.(option.key);
							}}
						>
							{#if option.glyph !== undefined}
								<Glyph
									spec={option.glyph}
									fallback={data ? option.label : sentence(option.label)}
								/>
							{/if}
							{data ? option.label : sentence(option.label)}
						</button>
					</li>
				{:else}
					<li class="px-3 py-2 label text-faint">no match</li>
				{/each}
			</ul>
			{#if footer}
				<div class="border-t border-line p-1">
					<!-- eslint-disable svelte/no-navigation-without-resolve -- callers pass resolved paths -->
					<a
						href={footer.href}
						class="flex h-control items-center gap-2 px-3 text-body text-dim hover:bg-line hover:text-fg"
						><Icon svg={footer.icon} size={12} />{footer.label}</a
					>
					<!-- eslint-enable svelte/no-navigation-without-resolve -->
				</div>
			{/if}
		</div>
	{/if}
</div>
