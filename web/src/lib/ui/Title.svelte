<script lang="ts">
	import type { Snippet } from "svelte";

	let { children, view }: { children: Snippet; view?: string } = $props();
</script>

<h1
	class="flex min-w-0 items-center gap-3 text-title font-medium text-fg"
	style:view-transition-name={view}
>
	{@render children()}
</h1>
