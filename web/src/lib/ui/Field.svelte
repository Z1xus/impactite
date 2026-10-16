<script lang="ts">
	import type { HTMLInputAttributes } from "svelte/elements";

	let {
		label,
		value = $bindable(""),
		...rest
	}: HTMLInputAttributes & { label: string; value?: string } = $props();
</script>

<label class="flex flex-col gap-2">
	<span class="label leading-[14px]">{label}</span>
	<input
		{...rest}
		bind:value
		class="h-9 rounded-px border border-edge bg-panel px-3 text-fg transition-colors duration-100 outline-none placeholder:text-faint hover:border-dim focus:border-cyan"
	/>
</label>
