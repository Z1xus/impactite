<script lang="ts">
	import { page } from "$app/state";
	import Loading from "#lib/dashboard/Loading.svelte";
	import Replays from "#lib/replay/Replays.svelte";
	import { findSite, session } from "#lib/session.svelte.ts";
	import Missing from "#lib/ui/Missing.svelte";
	import Shell from "#lib/ui/Shell.svelte";

	const domain = $derived(page.params.site ?? "");
	const site = $derived(findSite(domain));
</script>

<svelte:head>
	<title>{domain} replays</title>
</svelte:head>

<Shell>
	{#if site}
		<Replays {domain} />
	{:else if session.me}
		<Missing />
	{:else}
		<Loading />
	{/if}
</Shell>
