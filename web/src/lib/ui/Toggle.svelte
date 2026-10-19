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

	function move(event: KeyboardEvent, at: number) {
		const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
		if (!step) return;
		event.preventDefault();
		const next = options[(at + step + options.length) % options.length];
		value = next.key;
		(event.currentTarget as HTMLElement).parentElement
			?.querySelectorAll<HTMLElement>("[role=radio]")
			[options.indexOf(next)].focus();
	}
</script>

<div class="flex max-w-full shrink-0 flex-col gap-2">
	{#if label}
		<span class="label leading-[14px]">{label}</span>
	{/if}
	<div
		class="flex h-control w-fit max-w-full shrink-0 [scrollbar-width:none] overflow-x-auto rounded-px {plain
			? ''
			: 'ring-1 ring-control ring-inset'}"
		role="radiogroup"
		aria-label={label}
	>
		{#each options as option, at (option.key)}
			<button
				type="button"
				role="radio"
				aria-checked={value === option.key}
				tabindex={value === option.key ? 0 : -1}
				class="shrink-0 rounded-px px-3 font-sans text-body text-dim {value === option.key
					? 'bg-edge text-fg'
					: 'hover:text-fg'}"
				onclick={() => (value = option.key)}
				onkeydown={(event) => move(event, at)}
			>
				{sentence(option.label)}
			</button>
		{/each}
	</div>
</div>
