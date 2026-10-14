<script lang="ts">
	import { api } from "#lib/api.ts";
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
	});

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
				<Field label="name" placeholder="grafana" bind:value={keyName} required />
			</Form>
		</Section>

		<Message tone="bad">{error}</Message>
	</div>
</Shell>
