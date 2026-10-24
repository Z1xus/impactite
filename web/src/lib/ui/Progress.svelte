<script lang="ts">
	import { navigating } from "$app/state";

	let shown = $state(false);
	let full = $state(false);

	// fast navigations never show the bar
	$effect(() => {
		if (!navigating.to) return;
		const wait = setTimeout(() => {
			full = false;
			shown = true;
		}, 120);
		return () => {
			clearTimeout(wait);
			if (!shown) return;
			full = true;
			setTimeout(() => (shown = full = false), 240);
		};
	});
</script>

{#if shown}
	<div class="pointer-events-none fixed inset-x-0 top-0 z-50 h-[3px]" aria-hidden="true">
		<div class="h-full" class:full></div>
	</div>
{/if}

<style>
	div > div {
		width: 0;
		background: linear-gradient(
			90deg,
			var(--color-cyan-deep) calc(100% - 12px),
			var(--color-cyan) 0
		);
		animation: crawl 4s forwards;
	}
	.full {
		width: 100%;
		animation: done 240ms steps(4) forwards;
	}
	@keyframes crawl {
		0% {
			width: 0;
			animation-timing-function: steps(8);
		}
		15% {
			width: 45%;
			animation-timing-function: steps(8);
		}
		45% {
			width: 70%;
			animation-timing-function: steps(10);
		}
		100% {
			width: 88%;
		}
	}
	@keyframes done {
		60% {
			opacity: 1;
		}
		to {
			opacity: 0;
		}
	}
</style>
