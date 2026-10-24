<script lang="ts">
	import icon from "pixelarticons/svg/upload.svg?raw";
	import Icon from "./Icon.svelte";

	let {
		file = $bindable(null),
		accept,
		zone
	}: { file: File | null; accept?: string; zone?: { title: string; meta: string } } = $props();

	let over = $state(false);

	function drop(event: DragEvent) {
		event.preventDefault();
		over = false;
		file = event.dataTransfer?.files[0] ?? file;
	}
</script>

{#if zone}
	<label
		class="flex cursor-pointer flex-col items-center gap-3 rounded-px border border-dashed px-6 py-10 text-center focus-within:border-cyan {over
			? 'border-cyan bg-raised'
			: 'border-control hover:border-dim'}"
		ondragover={(event) => {
			event.preventDefault();
			over = true;
		}}
		ondragleave={() => (over = false)}
		ondrop={drop}
	>
		<Icon svg={icon} size={16} class="text-dim" />
		<span class="text-body text-fg">{file?.name ?? zone.title}</span>
		<span class="meta">{zone.meta}</span>
		<input
			type="file"
			class="sr-only"
			{accept}
			onchange={(event) => (file = event.currentTarget.files?.[0] ?? null)}
		/>
	</label>
{:else}
	<label
		class="flex h-control min-w-0 cursor-pointer items-center gap-2.5 rounded-px border border-control bg-panel px-3 focus-within:border-cyan hover:border-dim"
	>
		<Icon svg={icon} size={12} class="shrink-0 text-dim" />
		{#if file}
			<span class="truncate text-body text-fg">{file.name}</span>
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
{/if}
