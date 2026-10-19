<script lang="ts">
	import plus from "pixelarticons/svg/plus.svg?raw";
	import Icon from "./Icon.svelte";

	let { label, onclick }: { label: string; onclick: () => void } = $props();
</script>

<button
	type="button"
	aria-label={`Add ${label.toLowerCase()} panel`}
	class="flex h-full min-h-40 w-full flex-col items-center justify-center gap-3 rounded-px border border-dashed border-line text-dim hover:border-dim hover:bg-panel hover:text-fg"
	{onclick}
>
	<Icon svg={plus} size={24} />
	<span class="text-small">{label.charAt(0).toUpperCase() + label.slice(1)}</span>
</button>
