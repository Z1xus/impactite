<script lang="ts">
	import { resolve } from "$app/paths";
	import { cached } from "#lib/stats.ts";
	import Button from "#lib/ui/Button.svelte";
	import Empty from "#lib/ui/Empty.svelte";

	let {
		domain,
		text,
		compact = false
	}: { domain: string; text: string; compact?: boolean } = $props();

	let visited = $state<boolean | null>(null);
	$effect(() => {
		cached(domain, { metrics: ["pageviews"], date_range: "7d" }, false)
			.then((result) => (visited = (result.results[0]?.metrics[0] ?? 0) > 0))
			.catch(() => (visited = false));
	});
</script>

{#if visited !== null}
	<Empty {compact} {text}>
		{#if visited}
			<p class="-mt-2 max-w-md text-center text-body text-dim">
				If you moved from Plausible, your site might still be using the Plausible script.
				Switch to the impactite tracker to start recording.
			</p>
			<Button
				variant="quiet"
				href={resolve("/[site]/settings/[[section]]", {
					site: domain,
					section: "tracking"
				})}>Get the impactite tracker</Button
			>
		{/if}
	</Empty>
{/if}
