<script lang="ts" generics="T extends string">
	import { sentence } from "./text";
	let {
		value = $bindable(),
		options,
		label
	}: {
		value: T;
		options: { key: T; label: string }[];
		label?: string;
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
		class="flex min-h-control w-fit max-w-full shrink-0 flex-wrap rounded-px ring-1 ring-control ring-inset {options.length >
		3
			? 'max-sm:grid max-sm:w-full max-sm:grid-cols-2'
			: ''}"
		role="radiogroup"
		aria-label={label}
	>
		{#each options as option, at (option.key)}
			<button
				type="button"
				role="radio"
				aria-checked={value === option.key}
				tabindex={value === option.key ? 0 : -1}
				class="h-control flex-auto rounded-px px-2 font-sans text-body whitespace-nowrap text-dim sm:px-3 {value ===
				option.key
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
