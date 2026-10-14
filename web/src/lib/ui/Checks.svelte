<script lang="ts" generics="T extends string">
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
			class="flex h-8 cursor-pointer items-center gap-2 rounded-px border px-3 label transition-colors duration-100 {data
				? 'tracking-normal normal-case'
				: ''} {on
				? 'border-cyan/60 text-fg'
				: 'border-line hover:border-edge hover:text-fg'}"
			onclick={() => flip(option.key)}
		>
			<span class="h-2 w-2 rounded-[1px] {on ? 'bg-cyan' : 'border border-edge'}"></span>
			{option.label}
		</button>
	{/each}
</div>
