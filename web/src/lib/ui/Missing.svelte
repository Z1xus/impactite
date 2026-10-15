<script lang="ts">
	import { resolve } from "$app/paths";
	import Message from "./Message.svelte";

	let { text = "This site doesn't exist, or you don't have access to it." }: { text?: string } =
		$props();
</script>

<div class="flex flex-col items-center gap-4 py-24">
	<Message tone="dim">{text}</Message>
	<a href={resolve("/")} class="label text-dim transition-colors hover:text-fg">all sites</a>
</div>
