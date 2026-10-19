<script lang="ts">
	import { tick } from "svelte";
	import { goto, preloadCode } from "$app/navigation";
	import { page } from "$app/state";
	import { resolve } from "$app/paths";
	import { api, ApiError, type Me } from "#lib/api.ts";
	import { absorb } from "#lib/art/absorb.ts";
	import { get } from "#lib/passkey.ts";
	import { adopt, session } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	type Mode = "passkey" | "password" | "link";
	const MODES: { key: Mode; label: string }[] = [
		{ key: "passkey", label: "Use a passkey" },
		{ key: "password", label: "Use a password" },
		{ key: "link", label: "Use an email link" }
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

	let panel = $state<HTMLElement>();

	$effect(() => {
		void preloadCode("/");
	});

	async function finish(promise: Promise<{ me: Me }>) {
		const { me } = await promise;
		if (panel) await absorb(panel);
		adopt(me);
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
			if (err instanceof ApiError && err.data.totp) {
				asked = true;
				await tick();
				panel?.querySelector<HTMLInputElement>("[autocomplete=one-time-code]")?.focus();
			}
			throw err;
		}
	}

	async function withLink() {
		await api("/api/app/login/link", { email });
		sent = true;
	}
</script>

<svelte:head>
	<title>Sign in · impactite</title>
</svelte:head>

<Gate>
	<div class="flex flex-col gap-3" bind:this={panel}>
		{#if ssoError}
			<Message tone="bad">{ssoError}</Message>
		{/if}
		{#if mode === "password"}
			<Form block action="Sign in" submit={withPassword}>
				<Field
					label="email"
					type="email"
					bind:value={email}
					autocomplete="email"
					autofocus
					required
				/>
				<Field
					label="password"
					type="password"
					bind:value={secret}
					autocomplete="current-password"
					required
				/>
				<div
					class="grid overflow-hidden motion-safe:transition-[grid-template-rows,margin] motion-safe:duration-200 {asked
						? 'grid-rows-[1fr]'
						: '-mt-4 grid-rows-[0fr]'}"
					inert={!asked}
				>
					<div class="min-h-0">
						<Field
							label="code"
							placeholder="App or recovery code"
							bind:value={code}
							autocomplete="one-time-code"
							required={asked}
						/>
					</div>
				</div>
			</Form>
		{:else if mode === "link"}
			<Form block action="Email me a link" submit={withLink}>
				<Field
					label="email"
					type="email"
					bind:value={email}
					autocomplete="email"
					autofocus
					required
				/>
			</Form>
			{#if sent}
				<Message tone="cyan">If the account exists, the link is on its way.</Message>
			{/if}
		{:else}
			<Form block action="Sign in with passkey" submit={withPasskey} />
		{/if}
		{#if session.sso}
			<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- a server redirect, not a page -->
			<Button variant="quiet" href="/api/app/sso" class="w-full"
				>Sign in with {session.sso}</Button
			>
		{/if}
		<div class="flex flex-wrap justify-center gap-x-4">
			{#each MODES.filter((item) => item.key !== mode) as item (item.key)}
				<Button variant="plain" type="button" onclick={() => (mode = item.key)}>
					{item.label}
				</Button>
			{/each}
			{#if session.signup}
				<Button variant="plain" href={resolve("/signup")}>Create an account</Button>
			{/if}
		</div>
	</div>
</Gate>
