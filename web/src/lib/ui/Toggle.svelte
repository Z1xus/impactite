<script lang="ts" generics="T extends string">
	import { sentence } from "./text";
	let {
		value = $bindable(),
		options,
		label,
		plain = false
	}: {
		value: T;
		options: { key: T; label: string }[];
		label?: string;
		plain?: boolean;
	} = $props();
</script>

<div class="flex max-w-full shrink-0 flex-col gap-2">
	{#if label}
		<span class="label leading-[14px]">{label}</span>
	{/if}
	<div
		class="flex h-9 w-fit max-w-full shrink-0 [scrollbar-width:none] gap-0.5 overflow-x-auto rounded-px p-0.5 {plain
			? ''
			: 'border border-edge'}"
		role="radiogroup"
		aria-label={label}
	>
		{#each options as option (option.key)}
			<button
				type="button"
				role="radio"
				aria-checked={value === option.key}
				class="shrink-0 cursor-pointer rounded-px px-3 {plain
					? 'font-sans text-[13px] text-dim'
					: 'font-sans text-[14px] text-dim'} transition-colors duration-100 {value ===
				option.key
					? 'bg-edge text-fg'
					: 'hover:text-fg'}"
				onclick={() => (value = option.key)}
			>
				{sentence(option.label)}
			</button>
		{/each}
	</div>
</div>
