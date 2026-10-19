<script lang="ts">
	let {
		name,
		invited = false,
		size = 20
	}: { name: string; invited?: boolean; size?: number } = $props();
</script>

<span
	class="grid shrink-0 place-items-center rounded-full border font-mono leading-none uppercase {invited
		? 'border-dashed border-edge text-faint'
		: 'border-control bg-raised text-fg'}"
	style:width="{size}px"
	style:height="{size}px"
	style:font-size="{Math.round(size * 0.45)}px"
	aria-hidden="true">{name.trim().charAt(0)}</span
>
