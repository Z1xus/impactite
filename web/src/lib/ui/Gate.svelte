<script lang="ts">
	import type { Snippet } from "svelte";
	import Meteor from "#lib/art/Meteor.svelte";

	let { children }: { children?: Snippet } = $props();
</script>

<main class="grid min-h-dvh place-content-center justify-items-center px-6 py-16">
	<div class="flex w-full max-w-sm flex-col items-center gap-10">
		<div class="flex flex-col items-center gap-6">
			<Meteor size={168} pixel={3} />
			<span class="font-mono text-num tracking-tight">impactite</span>
		</div>
		<div class="min-h-96 w-full [&_p]:text-center">
			{@render children?.()}
		</div>
	</div>
</main>
