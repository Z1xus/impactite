<script lang="ts">
	import { collection } from "#lib/collection.svelte.ts";
	import { date } from "#lib/display.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Code from "#lib/ui/Code.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Section from "#lib/ui/Section.svelte";

	const keys = collection<{ id: number; name: string; prefix: string; created: number }>(
		() => "/api/app/keys",
		"keys"
	);
	let name = $state("");
	let fresh = $state("");

	$effect(() => {
		keys.load();
	});

	async function add() {
		fresh = (await keys.add<{ key: string }>({ name })).key;
		name = "";
	}

	const remove = (id: string | number) =>
		attempt(() => keys.remove(id), {
			title: "Delete this API key?",
			body: "Anything that uses it stops working.",
			action: "Delete key"
		});
</script>

<Section id="keys" label="api keys">
	<List
		items={keys.items?.map((key) => ({
			key: key.id,
			title: key.name,
			meta: [`${key.prefix}…`, `added ${date(key.created)}`]
		})) ?? null}
		source={keys}
		empty="No keys yet."
		{remove}
	/>
	{#if fresh}
		<Code code={fresh} />
		<Message>Copy this key now. You won't see it again.</Message>
		<span class="label">mcp</span>
		<Code
			code={`claude mcp add --transport http impactite ${location.origin}/mcp --header "Authorization: Bearer ${fresh}"`}
		/>
	{/if}
	<Form action="Add key" done="Added." submit={add} inline>
		<Field label="key name" placeholder="grafana" bind:value={name} required />
	</Form>
</Section>
