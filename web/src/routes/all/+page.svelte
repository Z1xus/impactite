<script lang="ts">
	import Dashboard from "#lib/Dashboard.svelte";
	import { currentTeam } from "#lib/session.svelte.ts";
	import Missing from "#lib/ui/Missing.svelte";
	import Shell from "#lib/ui/Shell.svelte";

	const team = $derived(currentTeam());
	// the server reads a team in the timezone of its oldest site
	const timezone = $derived(
		[...(team?.sites ?? [])].sort((a, b) => a.id - b.id)[0]?.timezone ?? "UTC"
	);
</script>

<Shell>
	{#if team?.sites.length}
		{#key team.id}
			<Dashboard domain="team:{team.id}" {timezone} />
		{/key}
	{:else if team}
		<Missing text="This team has no sites yet." />
	{/if}
</Shell>
