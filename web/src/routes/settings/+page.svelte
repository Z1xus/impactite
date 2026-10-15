<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import qrcode from "qrcode-generator";
	import { display, saveDisplay } from "#lib/display.svelte.ts";
	import { attempt, toast } from "#lib/toast.svelte.ts";
	import { api } from "#lib/api.ts";
	import { create } from "#lib/passkey.ts";
	import { adopt, refresh, session } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Code from "#lib/ui/Code.svelte";
	import Danger from "#lib/ui/Danger.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Settings from "#lib/ui/Settings.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Title from "#lib/ui/Title.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";

	type Passkey = { id: string; name: string; created_at: number; used_at: number | null };
	type Key = { id: number; name: string; prefix: string; created: number };
	type Session = { id: number; current: boolean; device: string; created: number; seen: number };

	const sections = [
		{ id: "profile", label: "profile" },
		{ id: "password", label: "password" },
		{ id: "two-factor", label: "two factor" },
		{ id: "display", label: "display" },
		{ id: "passkeys", label: "passkeys" },
		{ id: "sessions", label: "sessions" },
		{ id: "keys", label: "api keys" },
		{ id: "danger", label: "danger" }
	];

	let name = $state(session.me?.name ?? "");
	let email = $state(session.me?.email ?? "");
	let current = $state("");
	let next = $state("");
	let passkeys = $state<Passkey[] | null>(null);
	let sessions = $state<Session[] | null>(null);
	let keys = $state<Key[] | null>(null);
	let keyName = $state("");
	let fresh = $state("");
	let confirm = $state("");
	let pending = $state<{ secret: string; uri: string } | null>(null);
	let totpCode = $state("");
	let recovery = $state<string[] | null>(null);
	const qr = $derived.by(() => {
		if (!pending) return "";
		const code = qrcode(0, "M");
		code.addData(pending.uri);
		code.make();
		return code.createSvgTag({ cellSize: 4, margin: 0, scalable: true });
	});

	const date = (seconds: number) =>
		new Date(seconds * 1000).toLocaleDateString("en-US", {
			month: "short",
			day: "numeric",
			year: "numeric"
		});

	$effect(() => {
		api<{ passkeys: Passkey[] }>("/api/app/passkeys").then((r) => (passkeys = r.passkeys));
		api<{ sessions: Session[] }>("/api/app/me/sessions").then((r) => (sessions = r.sessions));
		api<{ keys: Key[] }>("/api/app/keys").then((r) => (keys = r.keys));
	});

	async function saveProfile() {
		await api("/api/app/me", { name, email }, "PATCH");
		await refresh();
		toast("Profile saved.");
	}

	async function savePassword() {
		await api("/api/app/me/password", { current, password: next }, "PUT");
		await refresh();
		current = next = "";
		toast("Password saved.");
	}

	const startTotp = () =>
		attempt(async () => {
			pending = await api("/api/app/totp/begin", {});
		});

	async function enableTotp() {
		if (!pending) return;
		recovery = (
			await api<{ recovery: string[] }>("/api/app/totp/enable", {
				secret: pending.secret,
				code: totpCode
			})
		).recovery;
		pending = null;
		totpCode = "";
		await refresh();
	}

	async function disableTotp() {
		await api("/api/app/totp/disable", { code: totpCode });
		totpCode = "";
		recovery = null;
		await refresh();
	}

	const addPasskey = () =>
		attempt(async () => {
			const { challenge, options } = await api<{ challenge: string; options: never }>(
				"/api/app/passkeys/new",
				{}
			);
			const credential = await create(options);
			passkeys = (
				await api<{ passkeys: Passkey[] }>("/api/app/passkeys", { challenge, credential })
			).passkeys;
		});

	const removePasskey = (id: string | number) =>
		attempt(async () => {
			passkeys = (
				await api<{ passkeys: Passkey[] }>(
					`/api/app/passkeys/${encodeURIComponent(id)}`,
					undefined,
					"DELETE"
				)
			).passkeys;
		});

	const endSession = (id: string | number) =>
		attempt(async () => {
			sessions = (
				await api<{ sessions: Session[] }>(
					`/api/app/me/sessions/${id}`,
					undefined,
					"DELETE"
				)
			).sessions;
		});

	const endOthers = () =>
		attempt(async () => {
			sessions = (
				await api<{ sessions: Session[] }>("/api/app/me/sessions", undefined, "DELETE")
			).sessions;
		});

	async function addKey() {
		const result = await api<{ keys: Key[]; key: string }>("/api/app/keys", { name: keyName });
		keys = result.keys;
		fresh = result.key;
		keyName = "";
	}

	const removeKey = (id: string | number) =>
		attempt(async () => {
			keys = (await api<{ keys: Key[] }>(`/api/app/keys/${id}`, undefined, "DELETE")).keys;
		});

	async function deleteAccount() {
		await api("/api/app/me", { email: confirm }, "DELETE");
		adopt(null);
		await goto(resolve("/login"));
	}
</script>

<Shell>
	<Settings {sections}>
		{#snippet title()}
			<Title>Account</Title>
		{/snippet}

		<Section id="profile" label="profile">
			<Form action="save" submit={saveProfile}>
				<div class="grid gap-4 sm:grid-cols-2">
					<Field label="name" bind:value={name} autocomplete="name" required />
					<Field
						label="email"
						type="email"
						bind:value={email}
						autocomplete="email"
						required
					/>
				</div>
			</Form>
		</Section>

		<Section id="password" label="password">
			<Form
				action={session.me?.password ? "change password" : "set password"}
				submit={savePassword}
			>
				<div class="grid gap-4 sm:grid-cols-2">
					{#if session.me?.password}
						<Field
							label="current password"
							type="password"
							bind:value={current}
							autocomplete="current-password"
							required
						/>
					{/if}
					<Field
						label="new password"
						type="password"
						bind:value={next}
						autocomplete="new-password"
						minlength={10}
						required
					/>
				</div>
			</Form>
		</Section>

		<Section id="two-factor" label="two factor">
			{#snippet actions()}
				{#if !session.me?.totp && !pending}
					<Button variant="quiet" onclick={startTotp}>set up</Button>
				{/if}
			{/snippet}
			{#if recovery}
				<span class="text-[14px] text-dim"
					>Two factor is on. Save these recovery codes now, each one signs you in once
					without the app.</span
				>
				<Code code={recovery.join("\n")} />
			{:else if session.me?.totp}
				<span class="text-[14px] text-dim"
					>Password sign in asks for a code from your authenticator app. Passkeys need no
					code.</span
				>
				<Form action="turn off" submit={disableTotp} inline>
					<Field
						label="code from your app or a recovery code"
						bind:value={totpCode}
						autocomplete="one-time-code"
						required
					/>
				</Form>
			{:else if pending}
				<div class="flex flex-wrap items-start gap-6">
					<!-- eslint-disable-next-line svelte/no-at-html-tags -- svg built from our own otpauth uri -->
					<div class="w-40 shrink-0 rounded-px bg-white p-2">{@html qr}</div>
					<div class="flex min-w-0 flex-1 flex-col gap-3">
						<span class="text-[14px] text-dim"
							>Scan this with your authenticator app, or enter the key by hand.</span
						>
						<Code code={pending.secret} />
						<Form action="turn on" submit={enableTotp} inline>
							<Field
								label="code from your app"
								bind:value={totpCode}
								autocomplete="one-time-code"
								inputmode="numeric"
								required
							/>
						</Form>
					</div>
				</div>
			{:else}
				<span class="text-[14px] text-dim"
					>Ask for a code from an authenticator app when you sign in with a password.</span
				>
			{/if}
		</Section>

		<Section id="display" label="display">
			<div class="flex flex-wrap gap-6">
				<Toggle
					label="theme"
					bind:value={() => display.theme, (theme) => saveDisplay({ theme })}
					options={[
						{ key: "dark", label: "dark" },
						{ key: "light", label: "light" },
						{ key: "system", label: "system" }
					]}
				/>
				<Toggle
					label="week starts on"
					bind:value={() => display.week, (week) => saveDisplay({ week })}
					options={[
						{ key: "monday", label: "monday" },
						{ key: "sunday", label: "sunday" }
					]}
				/>
				<Toggle
					label="clock"
					bind:value={() => display.clock, (clock) => saveDisplay({ clock })}
					options={[
						{ key: "24", label: "24 hour" },
						{ key: "12", label: "12 hour" }
					]}
				/>
				<Toggle
					label="numbers"
					bind:value={() => display.numbers, (numbers) => saveDisplay({ numbers })}
					options={[
						{ key: "en-US", label: "1,234.5" },
						{ key: "de-DE", label: "1.234,5" },
						{ key: "fr-FR", label: "1 234,5" }
					]}
				/>
			</div>
		</Section>

		<Section id="passkeys" label="passkeys">
			{#snippet actions()}
				<Button variant="quiet" onclick={addPasskey}>add passkey</Button>
			{/snippet}
			<List
				items={passkeys?.map((passkey) => ({
					key: passkey.id,
					title: passkey.name,
					meta: [
						`added ${date(passkey.created_at)}`,
						...(passkey.used_at ? [`used ${date(passkey.used_at)}`] : [])
					]
				})) ?? null}
				remove={removePasskey}
			/>
		</Section>

		<Section id="sessions" label="sessions">
			{#snippet actions()}
				{#if (sessions?.length ?? 0) > 1}
					<Button variant="quiet" onclick={endOthers}>sign out others</Button>
				{/if}
			{/snippet}
			<List
				items={sessions?.map((item) => ({
					key: item.id,
					title: item.device || "Unknown device",
					meta: item.current
						? ["this device"]
						: [
								`seen ${date(item.seen || item.created)}`,
								`since ${date(item.created)}`
							],
					removable: !item.current
				})) ?? null}
				remove={endSession}
			/>
		</Section>

		<Section id="keys" label="api keys">
			<List
				items={keys?.map((key) => ({
					key: key.id,
					title: key.name,
					meta: [`${key.prefix}…`, `added ${date(key.created)}`]
				})) ?? null}
				remove={removeKey}
			/>
			{#if fresh}
				<Code code={fresh} />
				<Message>Copy this key now. You won't see it again.</Message>
			{/if}
			<Form action="create key" submit={addKey} inline>
				<Field label="key name" placeholder="grafana" bind:value={keyName} required />
			</Form>
		</Section>

		<Section id="danger" label="danger" danger>
			<Danger
				title="Delete account"
				body="Deletes your account and your personal sites. You can't undo this."
			>
				<Form action="delete account" submit={deleteAccount} inline danger>
					<Field label="type your email to confirm" bind:value={confirm} />
				</Form>
			</Danger>
		</Section>
	</Settings>
</Shell>
