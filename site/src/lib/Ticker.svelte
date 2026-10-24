<script lang="ts">
	import Icon from "#lib/ui/Icon.svelte";
	import { FEATURES } from "./demo.ts";
</script>

<div class="ticker flex h-10 overflow-hidden border-y border-line">
	{#each [0, 1] as copy (copy)}
		<ul class="run flex shrink-0 items-center" aria-hidden={copy === 1}>
			{#each FEATURES as feature (feature.label)}
				<li class="flex items-center gap-2 px-5 whitespace-nowrap">
					<Icon svg={feature.icon} size={12} class="text-cyan" />
					<span class="font-mono text-small">{feature.label}</span>
				</li>
				<li class="size-1 shrink-0 bg-edge" aria-hidden="true"></li>
			{/each}
		</ul>
	{/each}
</div>

<style>
	/* stepped, so the fade stays on the pixel grid */
	.ticker {
		mask: linear-gradient(
			90deg,
			transparent 0 8px,
			#0006 8px 16px,
			#000b 16px 24px,
			#000 24px calc(100% - 24px),
			#000b calc(100% - 24px) calc(100% - 16px),
			#0006 calc(100% - 16px) calc(100% - 8px),
			transparent calc(100% - 8px)
		);
	}
	@media (prefers-reduced-motion: no-preference) {
		.run {
			animation: run 90s linear infinite;
		}
		.ticker:hover .run {
			animation-play-state: paused;
		}
	}
	@keyframes run {
		to {
			translate: -100% 0;
		}
	}
</style>
