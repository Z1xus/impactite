<script lang="ts">
	import close from "pixelarticons/svg/close.svg?raw";
	import type { Spec } from "#lib/icons/resolve.ts";
	import Glyph from "./Glyph.svelte";
	import Icon from "./Icon.svelte";

	let {
		label,
		value,
		icon,
		remove
	}: { label: string; value: string; icon?: Spec | null; remove: () => void } = $props();
</script>

<span class="inline-flex h-7 items-center gap-2 rounded-px border border-edge pr-1 pl-2.5">
	<span class="label">{label}</span>
	{#if icon}
		<Glyph spec={icon} />
	{/if}
	<span class="max-w-60 truncate text-[13px] text-fg">{value}</span>
	<button
		type="button"
		class="grid h-5 w-5 cursor-pointer place-items-center text-dim transition-colors duration-100 hover:text-fg"
		aria-label="remove filter"
		onclick={remove}
	>
		<Icon svg={close} size={12} />
	</button>
</span>
