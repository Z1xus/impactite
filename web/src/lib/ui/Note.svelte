<script lang="ts">
	import type { Snippet } from "svelte";

	let { children }: { children: Snippet } = $props();
</script>

<p class="text-[14px] text-dim">{@render children()}</p>
