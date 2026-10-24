<script lang="ts">
	let { class: className = "" }: { class?: string } = $props();
</script>

<span
	data-skeleton
	class="relative block overflow-hidden rounded-px dither [--dither-fill:var(--color-edge)] [--dither-gap:var(--color-line)] {className}"
></span>

<style>
	span::after {
		content: "";
		position: absolute;
		inset: 0;
		background-image:
			linear-gradient(45deg, var(--color-edge) 25%, transparent 25%),
			linear-gradient(-45deg, var(--color-edge) 25%, transparent 25%);
		background-color: var(--color-control);
		background-size: 4px 4px;
		opacity: 0.35;
		mask: linear-gradient(90deg, transparent 0 40%, #000 40% 60%, transparent 60%) 100% 0 / 300%
			100% no-repeat;
		animation: sweep 1.4s steps(28) infinite;
	}
	:global([data-style="plain"]) span::after {
		background-image: none;
	}
	@media (prefers-reduced-motion: reduce) {
		span::after {
			display: none;
		}
	}
	@keyframes sweep {
		to {
			mask-position: 0 0;
		}
	}
</style>
