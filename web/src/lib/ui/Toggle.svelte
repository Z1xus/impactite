<script lang="ts" generics="T extends string">
	let { value = $bindable(), options }: { value: T; options: { key: T; label: string }[] } =
		$props();
</script>

<div class="flex h-10 shrink-0 gap-0.5 rounded-px border border-edge p-0.5" role="radiogroup">
	{#each options as option (option.key)}
		<button
			type="button"
			role="radio"
			aria-checked={value === option.key}
			class="cursor-pointer rounded-px px-3 label transition-colors duration-100 {value ===
			option.key
				? 'bg-edge text-fg'
				: 'hover:text-fg'}"
			onclick={() => (value = option.key)}
		>
			{option.label}
		</button>
	{/each}
</div>
