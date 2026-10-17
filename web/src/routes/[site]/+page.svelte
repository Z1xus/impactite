<script lang="ts">
	import { page } from "$app/state";
	import Dashboard from "#lib/dashboard/Dashboard.svelte";
	import { findSite } from "#lib/session.svelte.ts";
	import Missing from "#lib/ui/Missing.svelte";
	import Shell from "#lib/ui/Shell.svelte";

	const domain = $derived(page.params.site ?? "");
	const site = $derived(findSite(domain));
</script>

<Shell>
	{#if site}
		{#key domain}
			<Dashboard {domain} timezone={site.timezone} />
		{/key}
	{:else}
		<Missing />
	{/if}
</Shell>
