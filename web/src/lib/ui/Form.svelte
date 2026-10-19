<script lang="ts">
	import type { Snippet } from "svelte";
	import { toast } from "#lib/toast.svelte.ts";
	import Button from "./Button.svelte";
	import Message from "./Message.svelte";

	let {
		action,
		submit,
		done,
		inline = false,
		block = false,
		danger = false,
		children
	}: {
		action: string;
		submit: () => Promise<unknown>;
		done?: string;
		inline?: boolean;
		block?: boolean;
		danger?: boolean;
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
			if (done) toast(done);
		} catch (err) {
			error = err instanceof Error ? err.message : "Something went wrong.";
		} finally {
			busy = false;
		}
	}
</script>

<form {onsubmit} class="flex flex-col {inline ? 'gap-2' : 'gap-4'}">
	{#if inline}
		<div
			class="flex flex-wrap items-end gap-2 [&>*:first-child]:min-w-48 [&>*:first-child]:flex-1"
		>
			{@render children?.()}
			<Button
				type="submit"
				variant={danger ? "danger" : "quiet"}
				{busy}
				class={danger ? "w-36" : ""}>{action}</Button
			>
		</div>
	{:else}
		{@render children?.()}
		<Button
			type="submit"
			variant={danger ? "danger" : "primary"}
			{busy}
			class={block ? "w-full" : "self-end"}>{action}</Button
		>
	{/if}
	<Message tone="bad">{error}</Message>
</form>
