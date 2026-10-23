<script lang="ts">
	import Tabs from "#lib/uptime/Tabs.svelte";
	import { currentTeam } from "#lib/session.svelte.ts";
	import StatusPages from "#lib/uptime/StatusPages.svelte";

	const team = $derived(currentTeam());
</script>

<Tabs>
	{#if team}
		<div class="max-w-form">
			<StatusPages team={team.id} />
		</div>
	{/if}
</Tabs>
