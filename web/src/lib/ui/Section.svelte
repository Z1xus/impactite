<script lang="ts">
	import type { Snippet } from "svelte";

	let {
		id,
		label,
		danger = false,
		actions,
		children
	}: {
		id?: string;
		label: string;
		danger?: boolean;
		actions?: Snippet;
		children: Snippet;
	} = $props();
</script>

<section
	{id}
	class="scroll-mt-6 rounded-px border bg-panel {danger ? 'border-bad/30' : 'border-line'}"
>
	<header
		class="flex min-h-13 items-center justify-between gap-4 border-b px-5 py-1.5 {danger
			? 'border-bad/30'
			: 'border-line'}"
	>
		<h2 class="label {danger ? 'text-bad' : 'text-fg'}">{label}</h2>
		{@render actions?.()}
	</header>
	<div class="flex flex-col gap-4 p-5">
		{@render children()}
	</div>
</section>
