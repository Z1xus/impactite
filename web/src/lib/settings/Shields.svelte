<script lang="ts">
	import { api } from "#lib/api.ts";
	import { collection } from "#lib/collection.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Code from "#lib/ui/Code.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Note from "#lib/ui/Note.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import { sitePath } from "./site.ts";

	let { domain }: { domain: string } = $props();

	type Kind = "ip" | "country" | "page" | "hostname" | "prop";
	type Shield = { id: number; kind: Kind; value: string };

	const KINDS: { key: Kind; label: string; meta: string; example: string }[] = [
		{ key: "ip", label: "ip", meta: "blocked ip", example: "203.0.113.0/24" },
		{ key: "country", label: "country", meta: "blocked country", example: "NL" },
		{ key: "page", label: "page", meta: "blocked page", example: "/admin/*" },
		{ key: "hostname", label: "hostname", meta: "allowed hostname", example: "*.example.com" },
		{ key: "prop", label: "prop", meta: "allowed prop", example: "plan" }
	];
	const IGNORE = 'localStorage.plausible_ignore = "true"';

	const shields = collection<Shield>(() => `${sitePath(domain)}/shields`, "shields");
	let adding = $state(false);
	let kind = $state<Kind>("ip");
	let value = $state("");
	const current = $derived(KINDS.find((item) => item.key === kind));

	$effect(() => {
		shields.load();
	});

	async function add() {
		await shields.add({ kind, value });
		value = "";
		adding = false;
	}

	async function useMyIp() {
		value = (await api<{ ip: string }>("/api/app/ip")).ip;
	}

	const remove = (id: string | number) => attempt(() => shields.remove(id));
</script>

<Dialog title="add rule" bind:open={adding}>
	<Form action="add rule" submit={add}>
		<Toggle label="kind" bind:value={kind} options={KINDS} />
		<div class="flex items-end gap-2">
			<div class="flex-1">
				<Field
					label={current?.meta ?? "value"}
					placeholder={current?.example}
					bind:value
					maxlength={300}
					required
				/>
			</div>
			{#if kind === "ip"}
				<Button type="button" variant="quiet" onclick={useMyIp}>Use my ip</Button>
			{/if}
		</div>
	</Form>
</Dialog>

<Section id="shields" label="shields">
	{#snippet actions()}
		<Button variant="quiet" onclick={() => (adding = true)}>Add rule</Button>
	{/snippet}
	{#if shields.items?.length === 0}
		<Note>Every visit counts.</Note>
	{/if}
	<List
		items={shields.items?.map((rule) => ({
			key: rule.id,
			title: rule.value,
			meta: [KINDS.find((item) => item.key === rule.kind)?.meta ?? rule.kind]
		})) ?? null}
		{remove}
	/>
	<Note>To skip your own visits, run this in your browser console.</Note>
	<Code code={IGNORE} />
</Section>
