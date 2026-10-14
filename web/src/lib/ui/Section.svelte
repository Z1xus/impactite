<script lang="ts">
	import type { Snippet } from "svelte";

	let { label, children }: { label: string; children: Snippet } = $props();
</script>

<section class="flex max-w-xl flex-col gap-5">
	<span class="label">{label}</span>
	{@render children()}
</section>
