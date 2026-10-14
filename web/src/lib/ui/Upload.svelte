<script lang="ts">
	import icon from "pixelarticons/svg/upload.svg?raw";
	import Icon from "./Icon.svelte";

	let { file = $bindable(null), accept }: { file: File | null; accept?: string } = $props();
</script>

<label
	class="flex h-10 min-w-0 cursor-pointer items-center gap-2.5 rounded-px border border-line bg-panel px-3 transition-colors duration-100 focus-within:border-cyan hover:border-edge"
>
	<Icon svg={icon} size={12} class="shrink-0 text-dim" />
	{#if file}
		<span class="truncate text-[14px] text-fg">{file.name}</span>
	{:else}
		<span class="label">choose file</span>
	{/if}
	<input
		type="file"
		class="sr-only"
		{accept}
		onchange={(event) => (file = event.currentTarget.files?.[0] ?? null)}
	/>
</label>
