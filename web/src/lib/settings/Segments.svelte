<script lang="ts">
	import { collection } from "#lib/collection.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import List from "#lib/ui/List.svelte";
	import Note from "#lib/ui/Note.svelte";
	import Section from "#lib/ui/Section.svelte";
	import { sitePath } from "./site.ts";

	let { domain, owner }: { domain: string; owner: boolean } = $props();

	type Segment = { id: number; name: string; shared: boolean; mine: boolean };

	const segments = collection<Segment>(() => `${sitePath(domain)}/segments`, "segments");

	$effect(() => {
		segments.load();
	});

	const remove = (id: string | number) =>
		attempt(() => segments.remove(id), {
			title: "Delete this segment?",
			body: "Everyone it's shared with loses it too.",
			action: "Delete segment"
		});
</script>

<Section id="segments" label="segments">
	{#if segments.items?.length === 0}<Note>No saved segments.</Note>{/if}
	<List
		items={segments.items?.map((segment) => ({
			key: segment.id,
			title: segment.name,
			meta: [segment.shared ? "shared" : "personal"],
			removable: segment.mine || owner
		})) ?? null}
		{remove}
	/>
</Section>
