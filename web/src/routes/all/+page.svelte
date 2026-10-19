<script lang="ts">
	import Dashboard from "#lib/dashboard/Dashboard.svelte";
	import { currentTeam, teamTimezone } from "#lib/session.svelte.ts";
	import Loading from "#lib/dashboard/Loading.svelte";
	import Missing from "#lib/ui/Missing.svelte";
	import Shell from "#lib/ui/Shell.svelte";

	const team = $derived(currentTeam());
</script>

<svelte:head>
	<title>All sites · impactite</title>
</svelte:head>

<Shell>
	{#if team?.sites.length}
		{#key team.id}
			<Dashboard domain="team:{team.id}" timezone={teamTimezone(team)} />
		{/key}
	{:else if team}
		<Missing text="This team has no sites yet." />
	{:else}
		<Loading />
	{/if}
</Shell>
