<script lang="ts">
	import type { Snippet } from "svelte";
	import Meteor from "#lib/art/Meteor.svelte";
	import { theme } from "#lib/theme.svelte.ts";
	import Logo from "./Logo.svelte";

	let { children }: { children?: Snippet } = $props();
</script>

<main
	class="flex min-h-dvh flex-col items-center justify-center gutter py-8 [@media(min-height:700px)]:py-16"
>
	<div class="flex w-full max-w-sm flex-col items-center gap-10">
		<div class="flex flex-col items-center gap-6">
			{#if theme.style === "plain"}
				<Logo size={88} />
			{:else}
				<Meteor size={168} pixel={3} />
			{/if}
			<span class="font-mono text-num tracking-tight">impactite</span>
		</div>
		<div class="min-h-96 w-full [&_p]:text-center">
			{@render children?.()}
		</div>
	</div>
</main>
