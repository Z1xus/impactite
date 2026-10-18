<script lang="ts">
	import { collection } from "#lib/collection.svelte.ts";
	import NewEvents from "./NewEvents.svelte";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Note from "#lib/ui/Note.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import { sitePath } from "./site.ts";
	import type { Goal } from "./types.ts";

	let { domain }: { domain: string } = $props();

	const goals = collection<Goal>(() => `${sitePath(domain)}/goals`, "goals");
	let suggestions = $state<{ name: string; count: number; revenue: boolean }[]>([]);
	let review = $state<NewEvents>();

	let adding = $state(false);
	let kind = $state<"event" | "page" | "revenue">("event");
	let value = $state("");
	let name = $state("");
	let scroll = $state("");
	let currency = $state("EUR");
	let renaming = $state<Goal | null>(null);
	let newName = $state("");

	$effect(() => {
		goals.load();
	});

	async function add() {
		await goals.add({
			page: kind === "page",
			value,
			name,
			scroll: kind === "page" ? Number(scroll) || 0 : 0,
			currency: kind === "revenue" ? currency : ""
		});
		value = name = scroll = "";
		adding = false;
		review?.load();
	}

	function rename(goal: Goal) {
		newName = goal.name;
		renaming = goal;
	}

	async function saveName() {
		if (!renaming) return;
		await goals.change(renaming.id, { name: newName });
		renaming = null;
	}

	const remove = (id: string | number) =>
		attempt(
			async () => {
				await goals.remove(id);
				review?.load();
			},
			{
				title: `Delete goal "${goals.items?.find((goal) => goal.id === id)?.name ?? ""}"?`,
				body: "Its conversions stop showing.",
				action: "Delete goal"
			}
		);
</script>

<Dialog title="add goal" bind:open={adding}>
	<Form action="Add goal" submit={add}>
		<Toggle
			label="kind"
			bind:value={kind}
			options={[
				{ key: "event", label: "event" },
				{ key: "page", label: "page" },
				{ key: "revenue", label: "revenue" }
			]}
		/>
		<div class="grid gap-4 {kind === 'event' ? '' : 'sm:grid-cols-2'}">
			<Field
				label={kind === "page" ? "path" : "event name"}
				placeholder={kind === "page" ? "/pricing" : "Signup"}
				bind:value
				maxlength={300}
				required
			/>
			{#if kind === "page"}
				<Field
					label="scrolled at least"
					type="number"
					min="0"
					max="100"
					placeholder="0 for any visit"
					bind:value={scroll}
				/>
			{:else if kind === "revenue"}
				<Field
					label="currency"
					placeholder="EUR"
					maxlength={3}
					bind:value={currency}
					required
				/>
			{/if}
		</div>
		<Field label="display name" placeholder="optional" maxlength={120} bind:value={name} />
	</Form>
</Dialog>

<Dialog
	title="rename goal"
	bind:open={
		() => renaming !== null,
		(open) => {
			if (!open) renaming = null;
		}
	}
>
	<Form action="Rename" submit={saveName}>
		<Field label="name" maxlength={120} bind:value={newName} required />
	</Form>
</Dialog>

<Section id="goals" label="goals">
	{#snippet actions()}
		<Button variant="quiet" onclick={() => (adding = true)}>Add goal</Button>
	{/snippet}
	<NewEvents bind:this={review} bind:events={suggestions} {domain} changed={goals.load} />
	{#if goals.items?.length === 0 && !suggestions.length}
		<Note>No goals yet.</Note>
	{/if}
	<List
		items={goals.items?.map((goal) => ({
			key: goal.id,
			title: goal.name,
			meta: [
				goal.currency ? `revenue in ${goal.currency}` : goal.page ? "page" : "event",
				...(goal.value !== goal.name ? [goal.value] : []),
				...(goal.scroll ? [`scrolled ${goal.scroll}%`] : [])
			]
		})) ?? null}
		{remove}
	>
		{#snippet control(key)}
			{@const goal = goals.items?.find((goal) => goal.id === key)}
			{#if goal}
				<Button variant="plain" onclick={() => rename(goal)}>Rename</Button>
			{/if}
		{/snippet}
	</List>
</Section>
