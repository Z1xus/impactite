<script lang="ts">
	import type { Snippet } from "svelte";
	import Button from "./Button.svelte";
	import Message from "./Message.svelte";

	let {
		action,
		submit,
		inline = false,
		children
	}: {
		action: string;
		submit: () => Promise<unknown>;
		inline?: boolean;
		children?: Snippet;
	} = $props();

	let busy = $state(false);
	let error = $state("");

	async function onsubmit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = "";
		try {
			await submit();
		} catch (err) {
			error = err instanceof Error ? err.message : "Something went wrong.";
		} finally {
			busy = false;
		}
	}
</script>

<form {onsubmit} class="flex flex-col {inline ? 'gap-2' : 'gap-5'}">
	{#if inline}
		<div class="flex items-end gap-2 [&>*:first-child]:flex-1">
			{@render children?.()}
			<Button type="submit" quiet {busy} class="h-10">{action}</Button>
		</div>
	{:else}
		{@render children?.()}
		<Button type="submit" {busy}>{action}</Button>
	{/if}
	<Message tone="bad">{error}</Message>
</form>
