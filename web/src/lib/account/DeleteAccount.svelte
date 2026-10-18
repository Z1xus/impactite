<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api } from "#lib/api.ts";
	import { adopt, session } from "#lib/session.svelte.ts";
	import Danger from "#lib/ui/Danger.svelte";
	import Section from "#lib/ui/Section.svelte";

	const email = $derived(session.me?.email ?? "");

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
		action="Delete account"
		submit={destroy}
		confirm={email}
	/>
</Section>
