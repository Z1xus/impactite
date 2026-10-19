<script lang="ts">
	import { tick, type Snippet } from "svelte";
	import close from "pixelarticons/svg/close.svg?raw";
	import Icon from "./Icon.svelte";

	let {
		open = $bindable(false),
		wide = false,
		title,
		children
	}: { open: boolean; wide?: boolean; title: string; children: Snippet } = $props();

	const id = $props.id();
	let dialog: HTMLDialogElement;
	let mounted = $state(false);

	$effect(() => {
		if (open && !dialog.open) {
			mounted = true;
			dialog.showModal();
			tick().then(() =>
				(
					dialog.querySelector<HTMLElement>("[autofocus]") ??
					dialog.querySelector<HTMLElement>(
						"input:not([type=checkbox]):not([type=radio]), textarea, select, [role=radio], button:not([aria-label=close])"
					)
				)?.focus()
			);
		}
		if (!open && dialog.open) dialog.close();
	});
</script>

<dialog
	bind:this={dialog}
	class="mx-auto mt-4 mb-auto max-h-[calc(100dvh-2rem)] [@media(min-height:700px)]:mt-[15dvh] [@media(min-height:700px)]:max-h-[calc(85dvh-1rem)] {wide
		? 'w-[min(60rem,calc(100vw-1.5rem))]'
		: 'w-[min(32rem,calc(100vw-1.5rem))]'} overflow-y-auto overscroll-contain rounded-px border border-edge bg-raised p-0 text-fg backdrop:bg-ink/70"
	aria-labelledby="{id}-title"
	onclose={() => {
		open = false;
		mounted = false;
	}}
	onclick={(event) => event.target === dialog && (open = false)}
>
	{#if mounted}
		<header
			class="sticky top-0 z-10 flex h-13 items-center justify-between gap-4 border-b border-edge bg-raised px-4 sm:px-5"
		>
			<h2 id="{id}-title" class="label text-fg">{title}</h2>
			<button
				type="button"
				class="-mr-2.5 grid h-control w-control place-items-center text-dim hover:text-fg"
				aria-label="close"
				title="close"
				onclick={() => (open = false)}
			>
				<Icon svg={close} size={16} />
			</button>
			<span class="rule"></span>
		</header>
		<div class="p-4 sm:p-5">
			{@render children()}
		</div>
	{/if}
</dialog>
