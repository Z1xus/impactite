<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api, type Me } from "#lib/api.ts";
	import { get } from "#lib/passkey.ts";
	import { adopt } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";

	let password = $state(false);
	let email = $state("");
	let secret = $state("");

	async function finish(promise: Promise<{ me: Me }>) {
		adopt((await promise).me);
		await goto(resolve("/"));
	}

	async function withPasskey() {
		const { challenge, options } = await api<{ challenge: string; options: never }>(
			"/api/app/passkeys/login/begin",
			{}
		);
		const credential = await get(options);
		await finish(api("/api/app/passkeys/login", { challenge, credential }));
	}

	function withPassword() {
		return finish(api("/api/app/login", { email, password: secret }));
	}
</script>

<Gate>
	<div class="flex flex-col gap-3">
		{#if password}
			<Form action="sign in" submit={withPassword}>
				<Field
					label="email"
					type="email"
					bind:value={email}
					autocomplete="email"
					required
				/>
				<Field
					label="password"
					type="password"
					bind:value={secret}
					autocomplete="current-password"
					required
				/>
			</Form>
		{:else}
			<Form action="sign in with passkey" submit={withPasskey} />
		{/if}
		<Button variant="plain" type="button" onclick={() => (password = !password)}>
			{password ? "use a passkey" : "use a password"}
		</Button>
	</div>
</Gate>
