<script lang="ts">
	import { resolve } from "$app/paths";
	import Button from "./Button.svelte";
	import Note from "./Note.svelte";

	let { text = "This site doesn't exist, or you don't have access to it." }: { text?: string } =
		$props();
</script>

<div class="flex flex-col items-center gap-5 py-24">
	<Note>{text}</Note>
	<Button variant="quiet" href={resolve("/")}>All sites</Button>
</div>
