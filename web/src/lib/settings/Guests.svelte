<script lang="ts">
	import { api } from "#lib/api.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Code from "#lib/ui/Code.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import { sitePath } from "./site.ts";

	let { domain }: { domain: string } = $props();

	type People = {
		guests: { id: number; name: string; email: string; role: string }[];
		invites: { id: number; email: string; role: string }[];
	};

	const path = $derived(`${sitePath(domain)}/guests`);
	let people = $state<People | null>(null);
	let email = $state("");
	let role = $state<"viewer" | "editor">("viewer");
	let link = $state("");

	$effect(() => {
		api<People>(path).then((result) => (people = result));
	});

	async function invite() {
		const result = await api<{ link: string; sent: boolean }>(path, { email, role });
		link = result.sent ? "" : result.link;
		email = "";
		people = await api<People>(path);
	}

	const drop = (url: string, question?: Parameters<typeof attempt>[1]) =>
		attempt(async () => {
			people = await api<People>(url, undefined, "DELETE");
		}, question);

	// guests and open invites share one list, their keys tell them apart
	const remove = (key: string | number) =>
		String(key).startsWith("g")
			? drop(`${path}/${String(key).slice(1)}`, {
					title: "Remove this guest?",
					body: "They lose access to this site.",
					action: "remove guest"
				})
			: drop(`${path}/invites/${String(key).slice(1)}`);
</script>

<Section id="guests" label="guests">
	<List
		items={people && [
			...people.guests.map((guest) => ({
				key: `g${guest.id}`,
				title: guest.name,
				avatar: guest.id,
				meta: [guest.email, guest.role]
			})),
			...people.invites.map((item) => ({
				key: `i${item.id}`,
				title: item.email,
				avatar: null,
				meta: ["invited", item.role]
			}))
		]}
		{remove}
	/>
	<Form action="invite" submit={invite} inline>
		<Field
			label="email"
			type="email"
			placeholder="ada@example.com"
			bind:value={email}
			required
		/>
		<Toggle
			bind:value={role}
			options={[
				{ key: "viewer", label: "viewer" },
				{ key: "editor", label: "editor" }
			]}
		/>
	</Form>
	{#if link}
		<Code code={link} />
		<Message>Mail is not set up, so share this link yourself.</Message>
	{/if}
</Section>
