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
	class="mx-auto mt-[15dvh] mb-auto {wide
		? 'w-[min(60rem,calc(100vw-2rem))]'
		: 'w-[min(32rem,calc(100vw-2rem))]'} overflow-visible rounded-px border border-edge bg-raised p-0 text-fg backdrop:bg-ink/70"
	aria-labelledby="{id}-title"
	onclose={() => {
		open = false;
		mounted = false;
	}}
	onclick={(event) => event.target === dialog && (open = false)}
>
	{#if mounted}
		<div class="p-5">
			<header class="mb-5 flex items-center justify-between gap-4">
				<h2 id="{id}-title" class="label text-fg">{title}</h2>
				<button
					type="button"
					class="-mr-2.5 grid h-control w-control place-items-center text-dim hover:text-fg"
					aria-label="close"
					title="close"
					onclick={() => (open = false)}
				>
					<Icon svg={close} size={14} />
				</button>
			</header>
			{@render children()}
		</div>
	{/if}
</dialog>
