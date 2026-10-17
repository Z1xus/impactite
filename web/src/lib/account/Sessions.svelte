<script lang="ts">
	import { api } from "#lib/api.ts";
	import { collection } from "#lib/collection.svelte.ts";
	import { date } from "#lib/display.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import List from "#lib/ui/List.svelte";
	import Section from "#lib/ui/Section.svelte";

	const sessions = collection<{
		id: number;
		current: boolean;
		device: string;
		created: number;
		seen: number;
	}>(() => "/api/app/me/sessions", "sessions");

	$effect(() => {
		sessions.load();
	});

	const endOthers = () =>
		attempt(async () => {
			await api("/api/app/me/sessions", undefined, "DELETE");
			await sessions.load();
		});

	const remove = (id: string | number) => attempt(() => sessions.remove(id));
</script>

<Section id="sessions" label="sessions">
	{#snippet actions()}
		{#if (sessions.items?.length ?? 0) > 1}
			<Button variant="quiet" onclick={endOthers}>Sign out others</Button>
		{/if}
	{/snippet}
	<List
		items={sessions.items?.map((item) => ({
			key: item.id,
			title: item.device || "Unknown device",
			meta: item.current
				? ["this device"]
				: [`seen ${date(item.seen || item.created)}`, `since ${date(item.created)}`],
			removable: !item.current
		})) ?? null}
		{remove}
	/>
</Section>
