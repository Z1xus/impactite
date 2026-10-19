<script lang="ts">
	let { class: className = "" }: { class?: string } = $props();
</script>

<span
	class="block rounded-px dither [--dither-fill:var(--color-edge)] [--dither-gap:var(--color-line)] {className}"
></span>
