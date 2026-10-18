<script lang="ts">
	import { untrack } from "svelte";
	import { api } from "#lib/api.ts";
	import { refresh } from "#lib/session.svelte.ts";
	import { toast } from "#lib/toast.svelte.ts";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Section from "#lib/ui/Section.svelte";
	import type { Team } from "./types.ts";

	let { team, reload }: { team: Team; reload: () => Promise<void> } = $props();

	let name = $state(untrack(() => team.name));

	async function rename() {
		await api(`/api/app/teams/${team.id}`, { name }, "PATCH");
		await refresh();
		await reload();
		toast("Saved.");
	}
</script>

<Section id="general" label="general">
	<Form action="rename" submit={rename} inline>
		<Field label="name" bind:value={name} required />
	</Form>
</Section>
