<script lang="ts">
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	const link = "label text-dim transition-colors hover:text-fg";
</script>

<Gate>
	<div class="flex flex-col items-center gap-4">
		<Message tone="dim"
			>{page.status === 404 ? "This page doesn't exist." : "This page hit an error."}</Message
		>
		{#if page.url.pathname.startsWith("/share/")}
			<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- a visitor on a shared dashboard stays on it -->
			<a href={page.url.pathname} data-sveltekit-reload class={link}>try again</a>
		{:else}
			<a href={resolve("/")} class={link}>go home</a>
		{/if}
	</div>
</Gate>
