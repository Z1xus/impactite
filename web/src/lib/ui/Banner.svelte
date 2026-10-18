<script lang="ts">
	import type { Snippet } from "svelte";
	import Note from "./Note.svelte";

	let {
		text,
		tone = "dim",
		children
	}: { text: string; tone?: "dim" | "warn"; children?: Snippet } = $props();
</script>

<div
	class="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 rounded-px border border-line bg-panel py-2 pl-4 {children
		? 'pr-2'
		: 'pr-4'}"
>
	<Note {tone}>{text}</Note>
	{@render children?.()}
</div>
