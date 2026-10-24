<script lang="ts">
	import { page } from "$app/state";
	import Loading from "#lib/dashboard/Loading.svelte";
	import Heatmaps from "#lib/heatmap/Heatmaps.svelte";
	import { findSite, session } from "#lib/session.svelte.ts";
	import Missing from "#lib/ui/Missing.svelte";
	import Shell from "#lib/ui/Shell.svelte";

	const domain = $derived(page.params.site ?? "");
	const site = $derived(findSite(domain));
</script>

<svelte:head>
	<title>{domain} heatmaps</title>
</svelte:head>

<Shell>
	{#if site}
		<Heatmaps {domain} timezone={site.timezone} />
	{:else if session.me}
		<Missing />
	{:else}
		<Loading />
	{/if}
</Shell>
