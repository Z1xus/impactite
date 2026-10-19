<script lang="ts">
	import { untrack } from "svelte";
	import { page } from "$app/state";
	import Dashboard from "#lib/dashboard/Dashboard.svelte";
	import { warm } from "#lib/dashboard/warm.ts";
	import { findSite, session } from "#lib/session.svelte.ts";
	import Loading from "#lib/dashboard/Loading.svelte";
	import Missing from "#lib/ui/Missing.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import type { PageProps } from "./$types";

	let { data }: PageProps = $props();

	const domain = $derived(page.params.site ?? "");
	const site = $derived(findSite(domain));
	// the dashboard mounts once its first requests are in, so late banners and panels never push it around
	let loaded = $state(untrack(() => data.warmed));

	$effect(() => {
		if (site && !loaded) void warm(domain, site.timezone).then(() => (loaded = true));
	});
</script>

<svelte:head>
	<title>{domain} · impactite</title>
</svelte:head>

<Shell>
	{#if site && loaded}
		<Dashboard {domain} timezone={site.timezone} />
	{:else if session.me && !site}
		<Missing />
	{:else}
		<Loading />
	{/if}
</Shell>
