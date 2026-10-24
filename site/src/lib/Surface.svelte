<script lang="ts">
	import type { Snippet } from "svelte";
	import Ramp from "./Ramp.svelte";

	let { raised = false, children }: { raised?: boolean; children: Snippet } = $props();
</script>

{#if raised}
	<Ramp />
	<div class="bg-raised">
		<div class="mx-auto max-w-6xl gutter py-section">{@render children()}</div>
	</div>
	<Ramp down />
{:else}
	<div class="mx-auto max-w-6xl gutter py-section">{@render children()}</div>
{/if}
