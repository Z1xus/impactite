<script lang="ts">
	import Tabs from "#lib/uptime/Tabs.svelte";
	import { currentTeam } from "#lib/session.svelte.ts";
	import Alerts from "#lib/uptime/Alerts.svelte";
	import { writes } from "#lib/uptime/types.ts";

	const team = $derived(currentTeam());
</script>

<Tabs>
	{#if team}
		<div class="max-w-form">
			<Alerts team={team.id} editable={writes(team.role)} />
		</div>
	{/if}
</Tabs>
