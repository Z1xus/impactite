<script lang="ts">
	import type { HTMLInputAttributes } from "svelte/elements";

	let {
		label,
		value = $bindable(""),
		size = "md",
		...rest
	}: Omit<HTMLInputAttributes, "size"> & {
		label: string;
		value?: string;
		size?: "md" | "sm";
	} = $props();
</script>

<label class="flex flex-col gap-2">
	<span class={size === "sm" ? "sr-only" : "label leading-[14px]"}>{label}</span>
	<input
		{...rest}
		bind:value
		class="rounded-px border border-control bg-panel text-fg {size === 'sm'
			? 'h-compact px-2.5 text-small'
			: 'h-control px-3 text-body'} transition-colors duration-100 outline-none placeholder:text-faint hover:border-dim focus:border-cyan"
	/>
</label>
