<script lang="ts">
	import { Tween, prefersReducedMotion } from "svelte/motion";
	import { cubicOut } from "svelte/easing";

	let {
		value,
		on,
		digits = 0,
		delay = 0
	}: { value: number; on: boolean; digits?: number; delay?: number } = $props();

	const tween = new Tween(0, { duration: 1400, easing: cubicOut });
	const format = (value: number) =>
		value.toLocaleString("en-US", {
			minimumFractionDigits: digits,
			maximumFractionDigits: digits
		});

	$effect(() => {
		const still = prefersReducedMotion.current;
		tween.set(on || still ? value : 0, on && !still ? { delay } : { duration: 0 });
	});
</script>

<span class="preview-animated num">{format(tween.current)}</span>
<span class="preview-static num">{format(value)}</span>
