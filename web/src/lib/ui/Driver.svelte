<script lang="ts">
	import type { Spec } from "#lib/icons/resolve.ts";
	import Glyph from "./Glyph.svelte";

	let {
		label,
		delta,
		icon,
		pick
	}: { label: string; delta: string; icon?: Spec | null; pick: () => void } = $props();
</script>

<button
	type="button"
	class="flex h-compact min-w-0 items-center gap-2 rounded-px px-2 hover:bg-raised"
	onclick={pick}
>
	{#if icon}
		<Glyph spec={icon} fallback={label} />
	{/if}
	<span class="max-w-48 truncate text-small text-fg">{label}</span>
	<span class="num text-label {delta.startsWith('-') ? 'text-bad' : 'text-cyan'}">{delta}</span>
</button>
