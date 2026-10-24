<script lang="ts">
	import { collection } from "#lib/collection.svelte.ts";
	import { get } from "#lib/api.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import List from "#lib/ui/List.svelte";
	import Section from "#lib/ui/Section.svelte";
	import { stamp } from "./format.ts";
	import MaintenanceDialog from "./MaintenanceDialog.svelte";
	import type { Monitor, Window } from "./types.ts";

	let { team, editable }: { team: number; editable: boolean } = $props();

	const windows = collection<Window>(
		() => "/api/app/uptime/maintenance",
		"maintenance",
		() => `?team=${team}`
	);
	let adding = $state(false);
	let names = $state.raw<Record<number, string>>({});

	$effect(() => {
		windows.load();
		get<{ monitors: Monitor[] }>(`/api/app/uptime?team=${team}`).then((reply) => {
			names = Object.fromEntries(reply.monitors.map((item) => [item.id, item.name]));
		});
	});

	const remove = (id: string | number) =>
		attempt(() => windows.remove(id), {
			title: "Cancel this maintenance?",
			body: "Monitors will go back to normal checks right away.",
			action: "Cancel maintenance"
		});
</script>

<MaintenanceDialog bind:open={adding} {team} saved={windows.load} />

<Section id="maintenance" label="maintenance">
	{#snippet actions()}
		{#if editable}
			<Button variant="quiet" onclick={() => (adding = true)}>Plan maintenance</Button>
		{/if}
	{/snippet}
	<List
		items={windows.items?.map((window) => ({
			key: window.id,
			title: window.note || "Maintenance",
			meta: [
				`${stamp(window.starts_at)} to ${stamp(window.ends_at)}`,
				window.monitors
					.map((id) => names[id] ?? "")
					.filter(Boolean)
					.join(", ")
			].filter(Boolean)
		})) ?? null}
		source={windows}
		empty="No maintenance planned."
		remove={editable ? remove : undefined}
	/>
</Section>
