<script lang="ts">
	import close from "pixelarticons/svg/close.svg?raw";
	import { collection } from "#lib/collection.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import List from "#lib/ui/List.svelte";
	import Note from "#lib/ui/Note.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import { sitePath } from "./site.ts";
	import type { Goal } from "./types.ts";

	let { domain }: { domain: string } = $props();

	type Funnel = { id: number; name: string; steps: number[] };

	const funnels = collection<Funnel>(() => `${sitePath(domain)}/funnels`, "funnels");
	const goals = collection<Goal>(() => `${sitePath(domain)}/goals`, "goals");

	let building = $state(false);
	let name = $state("");
	let steps = $state<number[]>([]);

	$effect(() => {
		funnels.load();
		goals.load();
	});

	const goalName = (id: number) => goals.items?.find((goal) => goal.id === id)?.name ?? "?";

	function start() {
		name = "";
		steps = (goals.items ?? []).slice(0, 2).map((goal) => goal.id);
		building = true;
	}

	async function add() {
		await funnels.add({ name, steps });
		building = false;
	}

	const remove = (id: string | number) =>
		attempt(() => funnels.remove(id), {
			title: `Delete funnel "${funnels.items?.find((funnel) => funnel.id === id)?.name ?? ""}"?`,
			action: "Delete funnel"
		});
</script>

<Dialog title="add funnel" bind:open={building}>
	<Form action="Add funnel" submit={add}>
		<Field label="name" placeholder="Checkout" bind:value={name} required />
		{#each steps as step, i (i)}
			<div class="flex items-end gap-2">
				<div class="flex-1">
					<Select
						label="step {i + 1}"
						value={step}
						options={(goals.items ?? []).map((goal) => ({
							key: goal.id,
							label: goal.name
						}))}
						data
						pick={(id) => (steps[i] = id)}
					/>
				</div>
				{#if steps.length > 2}
					<Button
						type="button"
						variant="icon"
						aria-label="remove step"
						title="remove step"
						onclick={() => (steps = steps.filter((_, j) => j !== i))}
						><Icon svg={close} size={14} /></Button
					>
				{/if}
			</div>
		{/each}
		{#if steps.length < 8}
			<div>
				<Button
					type="button"
					variant="quiet"
					onclick={() => (steps = [...steps, goals.items?.[0]?.id ?? 0])}>Add step</Button
				>
			</div>
		{/if}
	</Form>
</Dialog>

<Section id="funnels" label="funnels">
	{#snippet actions()}
		{#if (goals.items?.length ?? 0) >= 2}
			<Button variant="quiet" onclick={start}>Add funnel</Button>
		{/if}
	{/snippet}
	{#if goals.items && goals.items.length < 2}
		<Note>Needs two goals.</Note>
	{:else if funnels.items?.length === 0}
		<Note>No funnels yet.</Note>
	{/if}
	<List
		items={funnels.items?.map((funnel) => ({
			key: funnel.id,
			title: funnel.name,
			meta: funnel.steps.map((id, i) => `${i + 1}. ${goalName(id)}`)
		})) ?? null}
		{remove}
	/>
</Section>
