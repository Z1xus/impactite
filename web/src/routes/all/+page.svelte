<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { untrack } from "svelte";
	import { display } from "#lib/display.svelte.ts";
	import Dashboard from "#lib/dashboard/Dashboard.svelte";
	import { warm } from "#lib/dashboard/warm.ts";
	import { session, teamTimezone } from "#lib/session.svelte.ts";
	import Loading from "#lib/dashboard/Loading.svelte";
	import Empty from "#lib/ui/Empty.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import type { PageProps } from "./$types";

	let { data }: PageProps = $props();

	// the team stays while another one loads on the home page
	let id = $state(session.team);
	$effect(() => {
		id ??= session.team;
	});
	const team = $derived(session.me?.teams.find((item) => item.id === id));
	let loaded = $state(untrack(() => data.warmed));

	$effect(() => {
		if (display.all === "hide") void goto(resolve("/"), { replaceState: true });
	});

	$effect(() => {
		if (team?.sites.length && !loaded)
			void warm(`team:${team.id}`, teamTimezone(team)).then(() => (loaded = true));
	});
</script>

<svelte:head>
	<title>All sites</title>
</svelte:head>

<Shell>
	{#if team?.sites.length && loaded}
		{#key team.id}
			<Dashboard domain="team:{team.id}" timezone={teamTimezone(team)} />
		{/key}
	{:else if team && !team.sites.length}
		<Empty text="This team has no sites yet." />
	{:else}
		<Loading />
	{/if}
</Shell>
