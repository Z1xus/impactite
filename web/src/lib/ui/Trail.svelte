<script lang="ts">
	import chevron from "pixelarticons/svg/chevron-right.svg?raw";
	import Icon from "./Icon.svelte";

	let {
		items,
		current,
		pick
	}: { items: string[]; current: number; pick: (index: number) => void } = $props();
</script>

<nav
	aria-label="path"
	class="flex h-compact min-w-0 [scrollbar-width:none] items-center overflow-x-auto px-2"
>
	{#each items as item, i (i)}
		{#if i}
			<Icon svg={chevron} size={12} class="shrink-0 text-faint" />
		{/if}
		<button
			type="button"
			aria-current={i === current ? "step" : undefined}
			class="h-full shrink-0 truncate px-2 text-small {i === current
				? 'text-fg shadow-[inset_0_-2px_0_var(--color-cyan)]'
				: 'text-dim hover:text-fg'}"
			onclick={() => pick(i)}>{item}</button
		>
	{/each}
</nav>
