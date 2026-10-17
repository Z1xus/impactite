<script lang="ts">
	import type { Site } from "#lib/api.ts";
	import { collection } from "#lib/collection.svelte.ts";
	import { ask } from "#lib/ask.svelte.ts";
	import { attempt, copy, toast } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Code from "#lib/ui/Code.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Note from "#lib/ui/Note.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import { saveSite, sitePath } from "./site.ts";

	let { domain, site }: { domain: string; site: Site } = $props();

	type Link = { id: number; slug: string; name: string; locked: boolean };

	const links = collection<Link>(() => `${sitePath(domain)}/shares`, "shares");
	let adding = $state(false);
	let name = $state("");
	let password = $state("");

	const publicLink = $derived(links.items?.find((link) => link.slug === domain));
	const embedLink = $derived(
		publicLink ?? links.items?.find((link) => !link.locked) ?? links.items?.[0]
	);
	const url = (slug: string) => `${location.origin}/share/${slug}`;
	// split so the svelte parser does not end this block
	const scriptEnd = "</" + "script>";
	const counter = `<script>window.goatcounter = { no_onload: true }${scriptEnd}\n<script async src="${location.origin}/count.js" onload="goatcounter.visit_count({ append: 'body' })">${scriptEnd}`;

	$effect(() => {
		links.load();
	});

	async function add() {
		await links.add({ name, password });
		name = password = "";
		adding = false;
	}

	const setPublic = (on: boolean) =>
		attempt(async () => {
			if (on) await links.add({ name: "public", public: true });
			else if (
				publicLink &&
				(await ask({
					title: "Make the dashboard private?",
					body: "The public link stops working.",
					action: "make private"
				}))
			)
				await links.remove(publicLink.id);
		});

	const remove = (id: string | number) =>
		attempt(() => links.remove(id), {
			title: "Delete this shared link?",
			body: "Anyone with the link loses access.",
			action: "delete link"
		});
</script>

<Dialog title="add shared link" bind:open={adding}>
	<Form action="add link" submit={add}>
		<Field label="name" placeholder="Investors" bind:value={name} required />
		<Field label="password" type="password" placeholder="optional" bind:value={password} />
	</Form>
</Dialog>

<Section id="sharing" label="sharing">
	{#snippet actions()}
		<Button variant="quiet" onclick={() => (adding = true)}>Add link</Button>
	{/snippet}
	<div class="flex items-center justify-between gap-4">
		<Note>Public dashboard</Note>
		<Toggle
			bind:value={
				() => (publicLink ? "public" : "private"),
				(mode: "private" | "public") => setPublic(mode === "public")
			}
			options={[
				{ key: "private", label: "private" },
				{ key: "public", label: "public" }
			]}
		/>
	</div>
	{#if publicLink}
		<Code code={url(domain)} />
	{/if}
	<List
		items={links.items
			?.filter((link) => link.slug !== domain)
			.map((link) => ({
				key: link.id,
				title: link.name,
				meta: [url(link.slug), ...(link.locked ? ["password"] : [])]
			})) ?? null}
		{remove}
	>
		{#snippet control(key)}
			{@const link = links.items?.find((link) => link.id === key)}
			{#if link}
				<Button
					variant="plain"
					onclick={async () => (await copy(url(link.slug))) && toast("Link copied.")}
					>Copy</Button
				>
			{/if}
		{/snippet}
	</List>
	{#if embedLink}
		<Note>Embed the dashboard in another page.</Note>
		<Code
			code={`<iframe src="${url(embedLink.slug)}?embed=true" width="100%" height="1600" style="border:0" loading="lazy"></iframe>`}
		/>
	{/if}
	<div class="flex items-center justify-between gap-4">
		<Note>Visitor counter</Note>
		<Toggle
			bind:value={
				() => (site.counter ? "on" : "off"),
				(mode: "off" | "on") => saveSite(domain, { counter: mode === "on" })
			}
			options={[
				{ key: "off", label: "off" },
				{ key: "on", label: "on" }
			]}
		/>
	</div>
	{#if site.counter}
		<Code code={counter} />
	{/if}
</Section>
