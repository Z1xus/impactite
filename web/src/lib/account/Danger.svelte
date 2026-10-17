<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api } from "#lib/api.ts";
	import { adopt, session } from "#lib/session.svelte.ts";
	import Danger from "#lib/ui/Danger.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Section from "#lib/ui/Section.svelte";

	let email = $state("");

	async function destroy() {
		await api("/api/app/me", { email }, "DELETE");
		adopt(null);
		await goto(resolve("/login"));
	}
</script>

<Section id="danger" label="danger" danger>
	<Danger
		title="Delete account"
		body="This is permanent. You lose your account and your personal sites."
	>
		<Form action="delete account" submit={destroy} inline danger>
			<Field
				label="type your email to confirm"
				placeholder={session.me?.email}
				bind:value={email}
			/>
		</Form>
	</Danger>
</Section>
