<script lang="ts">
	import type { Snippet } from "svelte";
	import close from "pixelarticons/svg/close.svg?raw";
	import Icon from "./Icon.svelte";

	let {
		open = $bindable(false),
		wide = false,
		title,
		children
	}: { open: boolean; wide?: boolean; title: string; children: Snippet } = $props();

	let dialog: HTMLDialogElement;

	$effect(() => {
		if (open && !dialog.open) dialog.showModal();
		if (!open && dialog.open) dialog.close();
	});
</script>

<dialog
	bind:this={dialog}
	class="m-auto {wide
		? 'w-[min(60rem,calc(100vw-2rem))]'
		: 'w-[min(32rem,calc(100vw-2rem))]'} overflow-visible rounded-px border border-edge bg-raised p-5 text-fg backdrop:bg-ink/70"
	onclose={() => (open = false)}
	onclick={(event) => event.target === dialog && (open = false)}
>
	{#if open}
		<header class="mb-5 flex items-center justify-between gap-4">
			<span class="label text-fg">{title}</span>
			<button
				type="button"
				class="grid h-9 w-9 cursor-pointer place-items-center text-dim transition-colors duration-100 hover:text-fg"
				aria-label="close"
				title="close"
				onclick={() => (open = false)}
			>
				<Icon svg={close} size={14} />
			</button>
		</header>
		{@render children()}
	{/if}
</dialog>
