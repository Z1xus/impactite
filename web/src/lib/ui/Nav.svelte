<script lang="ts">
	let {
		items,
		label = "main"
	}: { items: { label: string; href: string; current: boolean }[]; label?: string } = $props();
</script>

<nav aria-label={label} class="flex items-stretch self-stretch">
	{#each items as item (item.href)}
		<!-- eslint-disable svelte/no-navigation-without-resolve -- callers pass resolved paths -->
		<a
			href={item.href}
			aria-current={item.current ? "page" : undefined}
			class="flex items-center px-2.5 font-sans text-body sm:px-3 {item.current
				? 'text-fg shadow-[inset_0_-2px_0_var(--color-cyan)]'
				: 'text-dim hover:text-fg'}">{item.label}</a
		>
		<!-- eslint-enable svelte/no-navigation-without-resolve -->
	{/each}
</nav>
