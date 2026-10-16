<script lang="ts">
	import { untrack } from "svelte";
	import { Tween } from "svelte/motion";
	import { cubicOut } from "svelte/easing";

	let {
		value,
		on,
		digits = 0,
		delay = 0,
		format = (value: number) =>
			value.toLocaleString("en-US", {
				minimumFractionDigits: digits,
				maximumFractionDigits: digits
			})
	}: {
		value: number;
		on: boolean;
		digits?: number;
		delay?: number;
		format?: (value: number) => string;
	} = $props();

	// the built page carries the real number, the browser counts up to it once it scrolls in
	const tween = new Tween(
		untrack(() => value),
		{ duration: 1400, easing: cubicOut }
	);

	$effect(() => {
		tween.set(on ? value : 0, on ? { delay } : { duration: 0 });
	});
</script>

<span class="num">{format(tween.current)}</span>
