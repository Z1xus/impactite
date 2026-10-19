<script lang="ts">
	import { resolve } from "$app/paths";
	import Button from "./Button.svelte";
	import Gate from "./Gate.svelte";
	import Message from "./Message.svelte";

	let { text, retry }: { text: string; retry?: () => void } = $props();
</script>

<Gate>
	<div class="flex flex-col items-center gap-4">
		<Message>{text}</Message>
		{#if retry}
			<Button variant="quiet" onclick={retry}>Retry</Button>
		{:else}
			<Button variant="quiet" href={resolve("/")}>Home</Button>
		{/if}
	</div>
</Gate>
