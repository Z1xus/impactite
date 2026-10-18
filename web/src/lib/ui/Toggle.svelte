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
		class="flex h-9 w-fit max-w-full shrink-0 [scrollbar-width:none] gap-0.5 overflow-x-auto rounded-px p-0.5 {plain
			? ''
			: 'border border-edge'}"
		role="radiogroup"
		aria-label={label}
	>
		{#each options as option, at (option.key)}
			<button
				type="button"
				role="radio"
				aria-checked={value === option.key}
				tabindex={value === option.key ? 0 : -1}
				class="shrink-0 cursor-pointer rounded-px px-3 {plain
					? 'font-sans text-small text-dim'
					: 'font-sans text-body text-dim'} transition-colors duration-100 {value ===
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
