<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api, type Me } from "#lib/api.ts";
	import { adopt } from "#lib/session.svelte.ts";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";

	let email = $state("");
	let password = $state("");

	async function submit() {
		const { me } = await api<{ me: Me }>("/api/app/login", { email, password });
		adopt(me);
		await goto(resolve("/"));
	}
</script>

<Gate>
	<Form action="sign in" {submit}>
		<Field label="email" type="email" bind:value={email} autocomplete="email" required />
		<Field
			label="password"
			type="password"
			bind:value={password}
			autocomplete="current-password"
			required
		/>
	</Form>
</Gate>
