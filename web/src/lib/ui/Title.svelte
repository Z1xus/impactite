<script lang="ts">
	import type { Snippet } from "svelte";

	let { children }: { children: Snippet } = $props();
</script>

<h1 class="flex min-w-0 items-center gap-3 text-[17px] font-medium text-fg">
	{@render children()}
</h1>
