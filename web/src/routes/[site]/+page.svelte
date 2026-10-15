<script lang="ts">
	import { page } from "$app/state";
	import Dashboard from "#lib/Dashboard.svelte";
	import { session } from "#lib/session.svelte.ts";
	import Shell from "#lib/ui/Shell.svelte";

	const domain = $derived(page.params.site ?? "");
	const site = $derived(
		session.me?.teams.flatMap((team) => team.sites).find((site) => site.domain === domain)
	);
</script>

<Shell>
	{#if site}
		{#key domain}
			<Dashboard {domain} timezone={site.timezone} />
		{/key}
	{/if}
</Shell>
