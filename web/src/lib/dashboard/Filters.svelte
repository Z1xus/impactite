<script lang="ts">
	import bookmark from "pixelarticons/svg/bookmark.svg?raw";
	import { page } from "$app/state";
	import { api } from "#lib/api.ts";
	import { ICONIC, icon } from "#lib/icons/resolve.ts";
	import { OPS, dimLabel, display, encodeFilter, type Filter } from "#lib/stats.ts";
	import Checks from "#lib/ui/Checks.svelte";
	import Chip from "#lib/ui/Chip.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Select from "#lib/ui/Select.svelte";

	let {
		domain,
		filters,
		segmented,
		editable,
		edit,
		remove,
		apply
	}: {
		domain: string;
		filters: Filter[];
		// segments belong to one site, and only members can use them
		segmented: boolean;
		editable: boolean;
		edit: (filter: Filter) => void;
		remove: (dim: string) => void;
		apply: (raw: string[]) => void;
	} = $props();

	type Segment = { id: number; name: string; filters: string[]; shared: boolean; mine: boolean };

	let segments = $state.raw<Segment[]>([]);
	let saving = $state(false);
	let name = $state("");
	let shared = $state<string[]>([]);

	const active = $derived(
		segments.find(
			(segment) => segment.filters.join("\n") === page.url.searchParams.getAll("f").join("\n")
		)
	);

	$effect(() => {
		if (!segmented) return;
		api<{ segments: Segment[] }>(`/api/app/sites/${domain}/segments`)
			.then((result) => (segments = result.segments))
			.catch(() => (segments = []));
	});

	function pick(key: string) {
		if (key === "save") saving = true;
		else apply(segments.find((item) => String(item.id) === key)?.filters ?? []);
	}

	async function save() {
		segments = (
			await api<{ segments: Segment[] }>(`/api/app/sites/${domain}/segments`, {
				name,
				filters: page.url.searchParams.getAll("f"),
				shared: shared.length > 0
			})
		).segments;
		saving = false;
		name = "";
	}
</script>

<Dialog title="save segment" bind:open={saving}>
	<Form action="Save segment" submit={save}>
		<Field label="segment name" maxlength={80} bind:value={name} required />
		{#if editable}
			<Checks
				bind:value={shared}
				options={[{ key: "shared", label: "share with everyone on this site" }]}
			/>
		{/if}
	</Form>
</Dialog>

{#if filters.length || (segmented && segments.length)}
	<div class="flex flex-wrap items-center gap-2">
		{#each filters as filter (encodeFilter(filter))}
			<Chip
				label={filter.op === "is"
					? dimLabel(filter.dim)
					: `${dimLabel(filter.dim)} ${OPS.find((op) => op.key === filter.op)?.label}`}
				value={filter.values.map((value) => display(filter.dim, value)).join(" or ")}
				icon={ICONIC.has(filter.dim) && filter.values.length === 1
					? icon(filter.dim, filter.values[0])
					: undefined}
				edit={() => edit(filter)}
				remove={() => remove(filter.dim)}
			/>
		{/each}
		{#if segmented && (segments.length || filters.length)}
			<Select
				value={active ? String(active.id) : ""}
				text={active?.name ?? "Segments"}
				plain
				icon={bookmark}
				data
				options={[
					...segments.map((segment) => ({
						key: String(segment.id),
						label: segment.name
					})),
					...(filters.length && !active
						? [{ key: "save", label: "Save as segment" }]
						: [])
				]}
				{pick}
			/>
		{/if}
	</div>
{/if}
