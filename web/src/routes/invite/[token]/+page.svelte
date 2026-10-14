<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { api, type Me } from "#lib/api.ts";
	import { adopt, session } from "#lib/session.svelte.ts";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";

	let name = $state("");
	let password = $state("");

	async function submit() {
		const body = session.me
			? { token: page.params.token }
			: { token: page.params.token, name, password };
		const { me } = await api<{ me: Me }>("/api/app/invites/accept", body);
		adopt(me);
		await goto(resolve("/"));
	}
</script>

<Gate>
	<Form action="join" {submit}>
		{#if !session.me}
			<Field label="name" bind:value={name} autocomplete="name" required />
			<Field
				label="password"
				type="password"
				bind:value={password}
				autocomplete="new-password"
				minlength={10}
				required
			/>
		{/if}
	</Form>
</Gate>
