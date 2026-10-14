<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { api, type Me } from "#lib/api.ts";
	import { adopt, session } from "#lib/session.svelte.ts";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	const token = page.url.searchParams.get("token") ?? "";
	let name = $state("");
	let email = $state("");
	let password = $state("");

	async function submit() {
		const { me } = await api<{ me: Me }>("/api/app/setup", { token, name, email, password });
		session.setup = false;
		adopt(me);
		await goto(resolve("/"));
	}
</script>

<Gate>
	{#if token}
		<Form action="create account" {submit}>
			<Field label="name" bind:value={name} autocomplete="name" required />
			<Field label="email" type="email" bind:value={email} autocomplete="email" required />
			<Field
				label="password"
				type="password"
				bind:value={password}
				autocomplete="new-password"
				minlength={10}
				required
			/>
		</Form>
	{:else}
		<Message>open the setup link from the server log</Message>
	{/if}
</Gate>
