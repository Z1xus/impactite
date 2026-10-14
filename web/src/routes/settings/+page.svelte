<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api } from "#lib/api.ts";
	import { adopt, refresh, session } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import { create } from "#lib/passkey.ts";
	import Code from "#lib/ui/Code.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Shell from "#lib/ui/Shell.svelte";

	type Passkey = { id: string; name: string; created_at: number; used_at: number | null };
	type Key = { id: number; name: string; prefix: string; created: number };
	type Session = { id: number; current: boolean; device: string; created: number; seen: number };

	let name = $state(session.me?.name ?? "");
	let email = $state(session.me?.email ?? "");
	let current = $state("");
	let next = $state("");
	let saved = $state("");
	let sessions = $state<Session[] | null>(null);
	let confirm = $state("");

	let passkeys = $state<Passkey[] | null>(null);
	let keys = $state<Key[] | null>(null);
	let keyName = $state("");
	let fresh = $state("");
	let error = $state("");

	const date = (seconds: number) =>
		new Date(seconds * 1000).toLocaleDateString("en-US", {
			month: "short",
			day: "numeric",
			year: "numeric"
		});

	$effect(() => {
		api<{ passkeys: Passkey[] }>("/api/app/passkeys").then((result) => {
			passkeys = result.passkeys;
		});
		api<{ keys: Key[] }>("/api/app/keys").then((result) => (keys = result.keys));
		api<{ sessions: Session[] }>("/api/app/me/sessions").then((r) => (sessions = r.sessions));
	});

	async function saveProfile() {
		await api("/api/app/me", { name, email }, "PATCH");
		await refresh();
		saved = "Saved.";
	}

	async function savePassword() {
		await api("/api/app/me/password", { current, password: next }, "PUT");
		await refresh();
		current = next = "";
		saved = "Password saved.";
	}

	async function endSession(id: string | number) {
		sessions = (
			await api<{ sessions: Session[] }>(`/api/app/me/sessions/${id}`, undefined, "DELETE")
		).sessions;
	}

	async function endOthers() {
		sessions = (await api<{ sessions: Session[] }>("/api/app/me/sessions", undefined, "DELETE"))
			.sessions;
	}

	async function deleteAccount() {
		await api("/api/app/me", { email: confirm }, "DELETE");
		adopt(null);
		await goto(resolve("/login"));
	}

	async function addKey() {
		const result = await api<{ keys: Key[]; key: string }>("/api/app/keys", { name: keyName });
		keys = result.keys;
		fresh = result.key;
		keyName = "";
	}

	async function removeKey(id: string | number) {
		error = "";
		try {
			keys = (await api<{ keys: Key[] }>(`/api/app/keys/${id}`, undefined, "DELETE")).keys;
		} catch (err) {
			error = err instanceof Error ? err.message : "Something went wrong.";
		}
	}

	async function add() {
		const { challenge, options } = await api<{ challenge: string; options: never }>(
			"/api/app/passkeys/new",
			{}
		);
		const credential = await create(options);
		passkeys = (
			await api<{ passkeys: Passkey[] }>("/api/app/passkeys", { challenge, credential })
		).passkeys;
	}

	async function remove(id: string | number) {
		error = "";
		try {
			passkeys = (
				await api<{ passkeys: Passkey[] }>(
					`/api/app/passkeys/${encodeURIComponent(id)}`,
					undefined,
					"DELETE"
				)
			).passkeys;
		} catch (err) {
			error = err instanceof Error ? err.message : "Something went wrong.";
		}
	}
</script>

<Shell>
	<div class="flex flex-col gap-12">
		<Section label="profile">
			<Form action="save" submit={saveProfile}>
				<Field label="name" bind:value={name} autocomplete="name" required />
				<Field
					label="email"
					type="email"
					bind:value={email}
					autocomplete="email"
					required
				/>
			</Form>
		</Section>

		<Section label="password">
			<Form
				action={session.me?.password ? "change password" : "set password"}
				submit={savePassword}
			>
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
			</Form>
			<Message tone="cyan">{saved}</Message>
		</Section>

		<Section label="sessions">
			<List
				items={sessions?.map((item) => ({
					key: item.id,
					title: item.device || "Unknown device",
					meta: item.current
						? "this device"
						: `seen ${date(item.seen || item.created)} · since ${date(item.created)}`,
					removable: !item.current
				})) ?? null}
				remove={endSession}
			/>
			{#if (sessions?.length ?? 0) > 1}
				<div class="w-fit">
					<Button variant="quiet" onclick={endOthers}>sign out other sessions</Button>
				</div>
			{/if}
		</Section>

		<Section label="passkeys">
			<List
				items={passkeys?.map((passkey) => ({
					key: passkey.id,
					title: passkey.name,
					meta: `added ${date(passkey.created_at)}${passkey.used_at ? ` · used ${date(passkey.used_at)}` : ""}`
				})) ?? null}
				{remove}
			/>
			<div class="max-w-60">
				<Form action="add passkey" submit={add} />
			</div>
		</Section>

		<Section label="api keys">
			<List
				items={keys?.map((key) => ({
					key: key.id,
					title: key.name,
					meta: `${key.prefix}… · added ${date(key.created)}`
				})) ?? null}
				remove={removeKey}
			/>
			{#if fresh}
				<Code code={fresh} />
				<Message>Copy this key now. It is shown only once.</Message>
			{/if}
			<Form action="create key" submit={addKey} inline>
				<Field label="key name" placeholder="grafana" bind:value={keyName} required />
			</Form>
		</Section>

		<Section label="delete account">
			<Form action="delete" submit={deleteAccount} inline>
				<Field label="type your email to delete your account" bind:value={confirm} />
			</Form>
		</Section>

		<Message tone="bad">{error}</Message>
	</div>
</Shell>
