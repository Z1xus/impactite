<script lang="ts">
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";
</script>

<Gate>
	<div class="flex flex-col items-center gap-4">
		<Message tone="dim"
			>{page.status === 404 ? "This page doesn't exist." : page.error?.message}</Message
		>
		<a href={resolve("/")} class="label text-dim transition-colors hover:text-fg">go home</a>
	</div>
</Gate>
