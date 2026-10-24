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
	<Empty
		{compact}
		text={visited ? `${text} Your site may load a script that can't record them.` : text}
	>
		{#if visited}
			<Button
				variant="quiet"
				href={resolve("/[site]/settings/[[section]]", {
					site: domain,
					section: "tracking"
				})}>Get the script</Button
			>
		{/if}
	</Empty>
{/if}
