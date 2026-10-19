<script lang="ts">
	import { page } from "$app/state";
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import alert from "pixelarticons/svg/square-alert.svg?raw";
	import sliders from "pixelarticons/svg/sliders.svg?raw";
	import users from "pixelarticons/svg/users.svg?raw";
	import { get } from "#lib/api.ts";
	import { currentTeam, session } from "#lib/session.svelte.ts";
	import General from "#lib/team/General.svelte";
	import Members from "#lib/team/Members.svelte";
	import TeamDanger from "#lib/team/TeamDanger.svelte";
	import type { Team } from "#lib/team/types.ts";
	import Missing from "#lib/ui/Missing.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";
	import Settings from "#lib/ui/Settings.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Title from "#lib/ui/Title.svelte";

	const current = $derived(currentTeam());
	const id = $derived(session.team);
	const owner = $derived(current?.role === "owner");
	const manager = $derived(!current || owner || current.role === "admin");
	const section = $derived(page.params.section ?? (manager ? "general" : "members"));
	const sections = $derived(
		[
			...(current && manager ? [{ id: "general", label: "general", icon: sliders }] : []),
			{ id: "members", label: "members", icon: users },
			{ id: "danger", label: "danger", icon: alert, danger: true }
		].map((item) => ({ ...item, href: resolve("/team/[[section]]", { section: item.id }) }))
	);

	let team = $state.raw<Team | null>(null);
	let failed = $state(false);

	$effect(() => {
		if (current && (current.personal || current.id === 0)) goto(resolve("/"));
		else if (id) load();
	});

	async function load() {
		failed = false;
		try {
			team = await get<Team>(`/api/app/teams/${id}`);
		} catch (err) {
			if (team) throw err;
			failed = true;
		}
	}
</script>

<Shell>
	{#if current && !sections.some((item) => item.id === section)}
		<Missing text="This settings page doesn't exist." />
	{:else}
		<Settings
			context={team?.name ?? current?.name ?? "Team"}
			{sections}
			current={section}
			loading={!team}
			retry={failed ? load : undefined}
		>
			{#snippet title()}
				<Title>
					{#if current}
						{team?.name ?? current.name}
					{:else}
						<Skeleton class="h-6 w-40" />
					{/if}
				</Title>
			{/snippet}

			{#if team}
				{#if section === "general"}
					<General {team} reload={load} />
				{:else if section === "members"}
					<Members {team} {owner} {manager} reload={load} />
				{:else}
					<TeamDanger {team} {owner} />
				{/if}
			{/if}
		</Settings>
	{/if}
</Shell>
