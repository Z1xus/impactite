<script lang="ts">
	import { Tween } from "svelte/motion";
	import { cubicOut } from "svelte/easing";

	let {
		value,
		format = (value: number) => Math.round(value).toLocaleString("en-US"),
		class: className = ""
	}: { value: number; format?: (value: number) => string; class?: string } = $props();

	const tween = new Tween(0, { duration: 320, easing: cubicOut });

	$effect(() => {
		const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
		tween.set(value, { duration: still ? 0 : 320 });
	});
</script>

<span class="num {className}">{format(tween.current)}</span>
