<script lang="ts">
	import { api } from "#lib/api.ts";
	import { refresh, session } from "#lib/session.svelte.ts";
	import { toast } from "#lib/toast.svelte.ts";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Section from "#lib/ui/Section.svelte";

	let name = $state(session.me?.name ?? "");
	let email = $state(session.me?.email ?? "");

	async function save() {
		const result = await api<{ pending?: string } | undefined>(
			"/api/app/me",
			{ name, email },
			"PATCH"
		);
		await refresh();
		// a new email waits for its confirmation link, the field shows the current one meanwhile
		if (result?.pending) {
			email = session.me?.email ?? email;
			toast(`We sent a link to ${result.pending} to confirm the change.`);
		} else toast("Profile saved.");
	}
</script>

<Section id="profile" label="profile">
	<Form action="save" submit={save}>
		<div class="grid gap-4 sm:grid-cols-2">
			<Field label="name" bind:value={name} autocomplete="name" required />
			<Field label="email" type="email" bind:value={email} autocomplete="email" required />
		</div>
	</Form>
</Section>
