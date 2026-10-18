<script lang="ts">
	import type { Snippet } from "svelte";
	import Note from "./Note.svelte";

	let { text, children }: { text: string; children?: Snippet } = $props();
</script>

<div class="flex flex-col items-center gap-5 py-24">
	<Note>{text}</Note>
	{@render children?.()}
</div>
