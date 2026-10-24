<script lang="ts">
	import { api } from "#lib/api.ts";
	import { collection } from "#lib/collection.svelte.ts";
	import { date } from "#lib/display.svelte.ts";
	import { create } from "#lib/passkey.ts";
	import { session } from "#lib/session.svelte.ts";
	import { attempt, toast } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import List from "#lib/ui/List.svelte";
	import Section from "#lib/ui/Section.svelte";

	const passkeys = collection<{
		id: string;
		name: string;
		created_at: number;
		used_at: number | null;
	}>(() => "/api/app/passkeys", "passkeys");

	$effect(() => {
		passkeys.load();
	});

	const add = () =>
		attempt(async () => {
			const { challenge, options } = await api<{ challenge: string; options: never }>(
				"/api/app/passkeys/new",
				{}
			);
			await passkeys.add({ challenge, credential: await create(options) });
			toast("Added.");
		});

	const remove = (id: string | number) =>
		attempt(() => passkeys.remove(id), {
			title: "Delete this passkey?",
			body: "You won't be able to sign in with this passkey anymore.",
			action: "Delete passkey"
		});
</script>

<Section id="passkeys" label="passkeys">
	{#snippet actions()}
		<Button variant="quiet" onclick={add}>Add passkey</Button>
	{/snippet}
	<List
		items={passkeys.items?.map((passkey, _, all) => {
			// the last way in can't go
			const only = all.length === 1 && !session.me?.password;
			return {
				key: passkey.id,
				title: passkey.name,
				removable: !only,
				meta: [
					`added ${date(passkey.created_at)}`,
					...(passkey.used_at ? [`used ${date(passkey.used_at)}`] : []),
					...(only ? ["your only sign-in"] : [])
				]
			};
		}) ?? null}
		source={passkeys}
		empty="No passkeys yet."
		{remove}
	/>
</Section>
