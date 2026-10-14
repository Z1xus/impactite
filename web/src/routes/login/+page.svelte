<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api, type Me } from "#lib/api.ts";
	import { get } from "#lib/passkey.ts";
	import { adopt, session } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	type Mode = "passkey" | "password" | "link";
	const MODES: { key: Mode; label: string }[] = [
		{ key: "passkey", label: "use a passkey" },
		{ key: "password", label: "use a password" },
		{ key: "link", label: "email me a link" }
	];

	let mode = $state<Mode>("passkey");
	let email = $state("");
	let secret = $state("");
	let sent = $state(false);

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

	async function withLink() {
		await api("/api/app/login/link", { email });
		sent = true;
	}
</script>

<Gate>
	<div class="flex flex-col gap-3">
		{#if mode === "password"}
			<Form block action="sign in" submit={withPassword}>
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
		{:else if mode === "link"}
			<Form block action="email me a link" submit={withLink}>
				<Field
					label="email"
					type="email"
					bind:value={email}
					autocomplete="email"
					required
				/>
			</Form>
			{#if sent}
				<Message tone="cyan">If the account exists, the link is on its way.</Message>
			{/if}
		{:else}
			<Form block action="sign in with passkey" submit={withPasskey} />
		{/if}
		<div class="flex flex-wrap justify-center gap-x-4">
			{#each MODES.filter((item) => item.key !== mode) as item (item.key)}
				<Button variant="plain" type="button" onclick={() => (mode = item.key)}>
					{item.label}
				</Button>
			{/each}
			{#if session.signup}
				<a
					href={resolve("/signup")}
					class="inline-flex h-9 items-center px-1 label text-dim transition-colors duration-100 hover:text-fg"
					>create an account</a
				>
			{/if}
		</div>
	</div>
</Gate>
