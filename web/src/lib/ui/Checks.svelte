<script lang="ts" generics="T extends string">
	import { sentence } from "./text";
	import check from "pixelarticons/svg/check.svg?raw";
	import Icon from "./Icon.svelte";

	let {
		value = $bindable(),
		options,
		data = false
	}: { value: T[]; options: { key: T; label: string }[]; data?: boolean } = $props();

	function flip(key: T) {
		value = value.includes(key) ? value.filter((item) => item !== key) : [...value, key];
	}
</script>

<div class="flex flex-wrap gap-2">
	{#each options as option (option.key)}
		{@const on = value.includes(option.key)}
		<button
			type="button"
			role="checkbox"
			aria-checked={on}
			class="flex h-9 cursor-pointer items-center gap-2.5 rounded-px border px-3 font-sans text-[14px] transition-colors duration-100 {data
				? 'tracking-normal normal-case'
				: ''} {on
				? 'border-cyan/70 bg-cyan/10 text-fg'
				: 'border-edge hover:border-dim hover:text-fg'}"
			onclick={() => flip(option.key)}
		>
			<span
				class="grid h-3.5 w-3.5 shrink-0 place-items-center rounded-[2px] border transition-colors duration-100 {on
					? 'border-cyan bg-cyan text-ink'
					: 'border-dim'}"
			>
				{#if on}
					<Icon svg={check} size={10} />
				{/if}
			</span>
			{data ? option.label : sentence(option.label)}
		</button>
	{/each}
</div>
