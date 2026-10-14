<script lang="ts">
	import close from "pixelarticons/svg/close.svg?raw";
	import Icon from "./Icon.svelte";

	let {
		items,
		remove
	}: {
		items: { key: string | number; title: string; meta?: string }[] | null;
		remove?: (key: string | number) => void;
	} = $props();
</script>

<ul class="flex flex-col rounded-px border border-line bg-panel">
	{#if items === null}
		<li class="h-14"></li>
	{:else if items.length === 0}
		<li class="flex h-14 items-center px-4 label">none</li>
	{:else}
		{#each items as item (item.key)}
			<li
				class="flex items-center justify-between gap-4 border-b border-line px-4 py-3 last:border-0"
			>
				<div class="flex min-w-0 flex-col gap-1.5">
					<span class="truncate text-[14px] text-fg">{item.title}</span>
					{#if item.meta}
						<span class="label">{item.meta}</span>
					{/if}
				</div>
				{#if remove}
					<button
						type="button"
						class="grid h-8 w-8 shrink-0 cursor-pointer place-items-center text-dim transition-colors duration-100 hover:text-bad"
						aria-label="remove {item.title}"
						onclick={() => remove(item.key)}
					>
						<Icon svg={close} size={14} />
					</button>
				{/if}
			</li>
		{/each}
	{/if}
</ul>
