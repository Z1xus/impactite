<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api } from "#lib/api.ts";
	import { refresh } from "#lib/session.svelte.ts";
	import Danger from "#lib/ui/Danger.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Section from "#lib/ui/Section.svelte";
	import { sitePath } from "./site.ts";

	let { domain }: { domain: string } = $props();

	let path = $state("");

	async function purge() {
		if (!path.startsWith("/")) throw new Error("Enter a path like /old-page.");
		await api(`${sitePath(domain)}/purge`, { page: path });
		path = "";
	}

	const wipe = () => api(`${sitePath(domain)}/reset`, {});

	async function destroy() {
		await api(sitePath(domain), undefined, "DELETE");
		await refresh();
		await goto(resolve("/"));
	}
</script>

<Section id="danger" label="danger" danger>
	<Danger
		title="Remove a page"
		body="All stats for this page will be deleted. Imported data will stay. You can't undo this action."
		action="Remove page"
		submit={purge}
	>
		<Field label="page path" placeholder="/old-page" bind:value={path} />
	</Danger>
	<Danger
		title="Reset stats"
		body="All stats for this site will be deleted. The site and its settings will stay. You can't undo this action."
		action="Reset stats"
		submit={wipe}
		confirm="reset"
	/>
	<Danger
		title="Delete site"
		body="This site and all its data will be deleted. You can't undo this action."
		action="Delete site"
		submit={destroy}
		confirm={domain}
	/>
</Section>
