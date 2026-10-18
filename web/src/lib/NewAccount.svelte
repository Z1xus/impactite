<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api, type Me } from "#lib/api.ts";
	import { create } from "#lib/passkey.ts";
	import { adopt, session } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";

	// a setup token makes the first account, without one this is an open sign-up
	let { token = "" }: { token?: string } = $props();
	let usePassword = $state(false);
	let name = $state("");
	let email = $state("");
	let password = $state("");

	async function submit() {
		const body: Record<string, unknown> = { token, name, email };
		if (usePassword) {
			body.password = password;
		} else {
			const { challenge, options } = await api<{ challenge: string; options: never }>(
				"/api/app/passkeys/signup",
				{ token, name, email, invite: false, open: !token }
			);
			body.passkey = { challenge, credential: await create(options) };
		}
		const { me } = await api<{ me: Me }>(token ? "/api/app/setup" : "/api/app/signup", body);
		if (token) session.setup = false;
		adopt(me);
		await goto(resolve("/"));
	}
</script>

<div class="flex flex-col gap-3">
	<Form block action={usePassword ? "Create account" : "Create passkey"} {submit}>
		<Field label="name" bind:value={name} autocomplete="name" required />
		<Field label="email" type="email" bind:value={email} autocomplete="email" required />
		{#if usePassword}
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
	<Button variant="plain" type="button" onclick={() => (usePassword = !usePassword)}>
		{usePassword ? "Use a passkey" : "Use a password instead"}
	</Button>
</div>
