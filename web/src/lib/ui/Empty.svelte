<script lang="ts">
	import type { Snippet } from "svelte";
	import Button from "./Button.svelte";
	import Note from "./Note.svelte";

	let {
		text = "Couldn't load.",
		retry,
		compact = false,
		children
	}: { text?: string; retry?: () => unknown; compact?: boolean; children?: Snippet } = $props();
</script>

<div data-empty class="flex flex-col items-center gap-4 {compact ? 'py-4' : 'py-24'}">
	<Note>{text}</Note>
	{#if retry}
		<Button variant="quiet" onclick={retry}>Retry</Button>
	{/if}
	{@render children?.()}
</div>
