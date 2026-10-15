<script lang="ts">
	import { Tween } from "svelte/motion";
	import { cubicOut } from "svelte/easing";

	let {
		value,
		on,
		digits = 0,
		delay = 0
	}: { value: number; on: boolean; digits?: number; delay?: number } = $props();

	const tween = new Tween(0, { duration: 1400, easing: cubicOut });

	$effect(() => {
		if (on) tween.set(value, { delay });
	});
</script>

<span class="num"
	>{tween.current.toLocaleString("en-US", {
		minimumFractionDigits: digits,
		maximumFractionDigits: digits
	})}</span
>
