<script lang="ts">
	import close from "pixelarticons/svg/close.svg?raw";
	import type { Spec } from "#lib/icons/resolve.ts";
	import Glyph from "./Glyph.svelte";
	import Icon from "./Icon.svelte";

	let {
		label,
		value,
		icon,
		edit,
		remove
	}: {
		label: string;
		value: string;
		icon?: Spec | null;
		edit?: () => void;
		remove: () => void;
	} = $props();
</script>

<span class="inline-flex h-control items-center gap-1 rounded-px border border-control pl-2.5">
	<button type="button" class="flex min-w-0 items-center gap-2" inert={!edit} onclick={edit}>
		<span class="label">{label}</span>
		{#if icon}
			<Glyph spec={icon} />
		{/if}
		<span class="max-w-60 truncate text-small text-fg">{value}</span>
	</button>
	<button
		type="button"
		class="grid h-full w-8 place-items-center text-dim hover:text-fg"
		aria-label="remove filter"
		title="remove filter"
		onclick={remove}
	>
		<Icon svg={close} size={12} />
	</button>
</span>
