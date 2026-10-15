<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { resolve } from "$app/paths";
	import { api, ApiError, type Me } from "#lib/api.ts";
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
	let code = $state("");
	let asked = $state(false);

	const SSO_ERRORS: Record<string, string> = {
		expired: "That sign-in took too long. Try again.",
		denied: "Sign-in was canceled.",
		email: "Your account there has no verified email.",
		invite: "There's no account for that email here yet. Ask an admin for an invite.",
		provider: "Sign-in didn't work. Try again, or ask an admin to check the SSO settings.",
		off: "Single sign-on isn't set up on this server."
	};
	const ssoError = $derived(SSO_ERRORS[page.url.searchParams.get("sso") ?? ""] ?? "");

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

	async function withPassword() {
		try {
			await finish(
				api("/api/app/login", { email, password: secret, ...(asked && { code }) })
			);
		} catch (err) {
			if (err instanceof ApiError && err.data.totp) asked = true;
			throw err;
		}
	}

	async function withLink() {
		await api("/api/app/login/link", { email });
		sent = true;
	}
</script>

<Gate>
	<div class="flex flex-col gap-3">
		{#if session.sso}
			<!-- eslint-disable svelte/no-navigation-without-resolve -- a server redirect, not a page -->
			<a
				href="/api/app/sso"
				class="flex h-9 w-full items-center justify-center rounded-px border border-edge label text-fg transition-colors duration-100 hover:border-dim"
				>sign in with {session.sso}</a
			>
			<!-- eslint-enable svelte/no-navigation-without-resolve -->
		{/if}
		{#if ssoError}
			<Message tone="bad">{ssoError}</Message>
		{/if}
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
				{#if asked}
					<Field
						label="code from your app or a recovery code"
						bind:value={code}
						autocomplete="one-time-code"
						required
					/>
				{/if}
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
