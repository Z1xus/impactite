<script lang="ts">
	import { untrack } from "svelte";
	import { api } from "#lib/api.ts";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import type { Note } from "./types.ts";

	let {
		domain,
		notes = $bindable(),
		day,
		editable
	}: { domain: string; notes: Note[]; day: string; editable: boolean } = $props();

	let date = $state(untrack(() => day));
	let text = $state("");

	const when = (day: string) =>
		new Date(`${day}T00:00:00Z`).toLocaleDateString("en-US", {
			timeZone: "UTC",
			month: "short",
			day: "numeric",
			year: "numeric"
		});

	async function add() {
		notes = (
			await api<{ notes: Note[] }>(`/api/app/sites/${domain}/notes`, { day: date, text })
		).notes;
		text = "";
	}

	async function remove(id: number) {
		notes = (
			await api<{ notes: Note[] }>(
				`/api/app/sites/${domain}/notes/${id}`,
				undefined,
				"DELETE"
			)
		).notes;
	}
</script>

<div class="flex flex-col gap-6">
	{#if editable}
		<Form action="Add note" submit={add}>
			<div class="grid gap-4 sm:grid-cols-[10rem_1fr]">
				<Field label="date" type="date" bind:value={date} required />
				<Field
					label="note"
					placeholder="Launched on Hacker News"
					maxlength={200}
					bind:value={text}
					autofocus
					required
				/>
			</div>
		</Form>
	{/if}
	<List
		items={[...notes].reverse().map((note) => ({
			key: note.id,
			title: note.text,
			meta: [when(note.day)]
		}))}
		empty="No notes yet."
		remove={editable ? remove : undefined}
	/>
</div>
