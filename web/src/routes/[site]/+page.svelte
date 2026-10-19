<script lang="ts">
	import { page } from "$app/state";
	import Dashboard from "#lib/dashboard/Dashboard.svelte";
	import { findSite, session } from "#lib/session.svelte.ts";
	import Loading from "#lib/dashboard/Loading.svelte";
	import Missing from "#lib/ui/Missing.svelte";
	import Shell from "#lib/ui/Shell.svelte";

	const domain = $derived(page.params.site ?? "");
	const site = $derived(findSite(domain));
</script>

<svelte:head>
	<title>{domain} · impactite</title>
</svelte:head>

<Shell>
	{#if site}
		<Dashboard {domain} timezone={site.timezone} />
	{:else if session.me}
		<Missing />
	{:else}
		<Loading />
	{/if}
</Shell>
