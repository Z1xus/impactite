<script lang="ts">
	import type { HTMLInputAttributes } from "svelte/elements";

	let {
		label,
		value = $bindable(""),
		...rest
	}: HTMLInputAttributes & { label: string; value?: string } = $props();
</script>

<label class="flex flex-col gap-2">
	<span class="label">{label}</span>
	<input
		{...rest}
		bind:value
		class="h-10 rounded-px border border-line bg-panel px-3 text-fg transition-colors duration-100 outline-none placeholder:text-faint hover:border-edge focus:border-cyan"
	/>
</label>
