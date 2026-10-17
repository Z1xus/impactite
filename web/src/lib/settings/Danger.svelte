<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api } from "#lib/api.ts";
	import { refresh } from "#lib/session.svelte.ts";
	import Danger from "#lib/ui/Danger.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Section from "#lib/ui/Section.svelte";
	import { sitePath } from "./site.ts";

	let { domain }: { domain: string } = $props();

	let path = $state("");
	let reset = $state("");
	let typed = $state("");

	async function purge() {
		if (!path.startsWith("/")) throw new Error("Use a path like /old-page.");
		await api(`${sitePath(domain)}/purge`, { page: path });
		path = "";
	}

	async function wipe() {
		if (reset.trim().toLowerCase() !== "reset") throw new Error("Type reset to confirm.");
		await api(`${sitePath(domain)}/reset`, {});
		reset = "";
	}

	async function destroy() {
		if (typed.trim().toLowerCase() !== domain) throw new Error(`Type ${domain} to confirm.`);
		await api(sitePath(domain), undefined, "DELETE");
		await refresh();
		await goto(resolve("/"));
	}
</script>

<Section id="danger" label="danger" danger>
	<Danger
		title="Remove a page"
		body="All stats for this page are deleted permanently. Imported data is kept."
	>
		<Form action="remove page" submit={purge} inline danger>
			<Field label="page path" placeholder="/old-page" bind:value={path} />
		</Form>
	</Danger>
	<Danger
		title="Reset stats"
		body="All stats are deleted permanently. The site and its settings are kept."
	>
		<Form action="reset stats" submit={wipe} inline danger>
			<Field label="type reset to confirm" placeholder="reset" bind:value={reset} />
		</Form>
	</Danger>
	<Danger title="Delete site" body="This is permanent. You lose the site and all its data.">
		<Form action="delete site" submit={destroy} inline danger>
			<Field label="type the domain to confirm" placeholder={domain} bind:value={typed} />
		</Form>
	</Danger>
</Section>
