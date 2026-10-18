<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api } from "#lib/api.ts";
	import { ask } from "#lib/ask.svelte.ts";
	import { refresh, session } from "#lib/session.svelte.ts";
	import Danger from "#lib/ui/Danger.svelte";
	import Section from "#lib/ui/Section.svelte";
	import type { Team } from "./types.ts";

	let { team, owner }: { team: Team; owner: boolean } = $props();

	async function leave() {
		const yes = await ask({
			title: `Leave ${team.name}?`,
			body: "You'll lose access to its sites.",
			action: "Leave team"
		});
		if (!yes) return;
		await api(`/api/app/teams/${team.id}/members/${session.me?.id}`, undefined, "DELETE");
		await refresh();
		await goto(resolve("/"));
	}

	async function destroy() {
		await api(`/api/app/teams/${team.id}`, undefined, "DELETE");
		await refresh();
		await goto(resolve("/"));
	}
</script>

<Section id="danger" label="danger" danger>
	{#if owner}
		<Danger
			title="Delete team"
			body="This is permanent. You lose the team and all its sites."
			action="Delete team"
			submit={destroy}
			confirm={team.name}
		/>
	{:else}
		<Danger
			title="Leave team"
			body="You lose access to its sites until someone invites you again."
			action="Leave team"
			submit={leave}
		/>
	{/if}
</Section>
