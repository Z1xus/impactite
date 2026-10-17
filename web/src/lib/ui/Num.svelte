<script lang="ts">
	import { untrack } from "svelte";
	import { number } from "#lib/display.svelte.ts";
	import { Tween } from "svelte/motion";
	import { cubicOut } from "svelte/easing";

	let {
		value,
		format = (value: number) => number(Math.round(value)),
		class: className = ""
	}: { value: number; format?: (value: number) => string; class?: string } = $props();

	const tween = new Tween(
		untrack(() => value),
		{ duration: 320, easing: cubicOut }
	);

	$effect(() => {
		const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
		tween.set(value, { duration: still ? 0 : 320 });
	});
</script>

<span class="num {className}">{format(tween.current)}</span>
