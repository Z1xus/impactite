<script lang="ts">
	import { flip } from "svelte/animate";
	import { fly } from "svelte/transition";
	import close from "pixelarticons/svg/close.svg?raw";
	import { dismiss, toasts } from "#lib/toast.svelte.ts";
	import Icon from "./Icon.svelte";
</script>

<div
	class="fixed right-4 bottom-4 z-50 flex w-[min(24rem,calc(100vw-2rem))] flex-col gap-2"
	aria-live="polite"
>
	{#each toasts as item (item.id)}
		<div
			animate:flip={{ duration: 150 }}
			transition:fly={{ y: 8, duration: 150 }}
			class="flex min-h-9 items-center gap-3 rounded-px border bg-raised pl-3 text-[14px] {item.bad
				? 'border-bad/50 text-bad'
				: 'border-edge text-fg'}"
		>
			<span class="flex-1 py-2">{item.text}</span>
			<button
				type="button"
				class="grid h-9 w-9 shrink-0 cursor-pointer place-items-center text-dim transition-colors duration-100 hover:text-fg"
				aria-label="dismiss"
				onclick={() => dismiss(item.id)}
			>
				<Icon svg={close} size={14} />
			</button>
		</div>
	{/each}
</div>
