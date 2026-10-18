<script lang="ts">
	import type { Snippet } from "svelte";

	let { tone = "dim", children }: { tone?: "dim" | "warn"; children: Snippet } = $props();
</script>

<p class="text-body {tone === 'warn' ? 'text-warn' : 'text-dim'}">{@render children()}</p>
