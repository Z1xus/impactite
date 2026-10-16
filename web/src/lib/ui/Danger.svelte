<script lang="ts">
	import type { Snippet } from "svelte";

	let { title, body, children }: { title: string; body: string; children: Snippet } = $props();
</script>

<div class="flex flex-col gap-4 border-b border-line pb-4 last:border-0 last:pb-0">
	<div class="flex flex-col gap-1">
		<span class="text-[14px] text-fg">{title}</span>
		<span class="text-[14px] text-dim">{body}</span>
	</div>
	{@render children()}
</div>
