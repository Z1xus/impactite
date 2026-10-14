<script lang="ts">
	import type { Snippet } from "svelte";

	let { open = $bindable(false), children }: { open: boolean; children: Snippet } = $props();

	let dialog: HTMLDialogElement;

	$effect(() => {
		if (open && !dialog.open) dialog.showModal();
		if (!open && dialog.open) dialog.close();
	});
</script>

<dialog
	bind:this={dialog}
	class="m-auto w-[min(32rem,calc(100vw-2rem))] rounded-px border border-edge bg-raised p-5 text-fg backdrop:bg-ink/70"
	onclose={() => (open = false)}
	onclick={(event) => event.target === dialog && (open = false)}
>
	{#if open}
		{@render children()}
	{/if}
</dialog>
