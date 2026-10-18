<script lang="ts">
	import type { Snippet } from "svelte";
	import Field from "./Field.svelte";
	import Form from "./Form.svelte";
	import Note from "./Note.svelte";

	let {
		title,
		body,
		action,
		submit,
		confirm,
		children
	}: {
		title: string;
		body: string;
		action: string;
		submit: () => Promise<unknown>;
		confirm?: string;
		children?: Snippet;
	} = $props();

	let typed = $state("");

	async function run() {
		if (confirm && typed.trim().toLowerCase() !== confirm.toLowerCase())
			throw new Error(`Type ${confirm} to confirm.`);
		await submit();
		typed = "";
	}
</script>

<div class="flex flex-col gap-4 border-b border-line pb-4 last:border-0 last:pb-0">
	<div class="flex flex-col gap-1">
		<span class="text-body text-fg">{title}</span>
		<Note>{body}</Note>
	</div>
	<Form {action} submit={run} inline danger>
		{@render children?.()}
		{#if confirm}
			<Field label="type {confirm} to confirm" placeholder={confirm} bind:value={typed} />
		{/if}
	</Form>
</div>
