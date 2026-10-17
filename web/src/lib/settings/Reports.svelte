<script lang="ts">
	import { collection } from "#lib/collection.svelte.ts";
	import { session } from "#lib/session.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Note from "#lib/ui/Note.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import { sitePath } from "./site.ts";

	let { domain }: { domain: string } = $props();

	type Kind = "daily" | "weekly" | "biweekly" | "monthly" | "spike" | "drop" | "export";
	type Report = { id: number; kind: Kind; email: string; threshold: number };

	const KINDS: { key: Kind; label: string; meta: (threshold: number) => string }[] = [
		{ key: "daily", label: "daily report", meta: () => "daily report" },
		{ key: "weekly", label: "weekly report", meta: () => "weekly report" },
		{ key: "biweekly", label: "report every two weeks", meta: () => "report every two weeks" },
		{ key: "monthly", label: "monthly report", meta: () => "monthly report" },
		{
			key: "spike",
			label: "traffic spike alert",
			meta: (n) => `alert at ${n} current visitors`
		},
		{
			key: "drop",
			label: "traffic drop alert",
			meta: (n) => `alert under ${n} visitors a day`
		},
		{ key: "export", label: "daily raw export", meta: () => "daily raw export" }
	];

	const reports = collection<Report>(() => `${sitePath(domain)}/reports`, "reports");
	let adding = $state(false);
	let kind = $state<Kind>("weekly");
	let target = $state(session.me?.email ?? "");
	let threshold = $state("");
	const limit = $derived(kind === "spike" || kind === "drop");

	$effect(() => {
		reports.load();
	});

	// an export goes to a bucket, everything else to a person
	$effect(() => {
		const bucket = target.startsWith("s3");
		if (kind === "export" && !bucket) target = "";
		if (kind !== "export" && bucket) target = session.me?.email ?? "";
	});

	async function add() {
		await reports.add({ kind, email: target, threshold: Number(threshold) || 0 });
		adding = false;
	}

	const remove = (id: string | number) => attempt(() => reports.remove(id));
</script>

<Dialog title="add report" bind:open={adding}>
	<Form action="add report" submit={add}>
		<Select label="kind" value={kind} options={KINDS} pick={(key) => (kind = key)} />
		<div class="grid gap-4 {limit ? 'sm:grid-cols-2' : ''}">
			<Field
				label={kind === "export" ? "s3 bucket" : "send to"}
				placeholder={kind === "export"
					? "s3://KEY:SECRET@host/bucket/folder"
					: "email or webhook url"}
				bind:value={target}
				required
			/>
			{#if limit}
				<Field
					label={kind === "spike" ? "current visitors at least" : "visitors a day under"}
					type="number"
					min="1"
					max="1000000000"
					bind:value={threshold}
					required
				/>
			{/if}
		</div>
	</Form>
</Dialog>

<Section id="reports" label="reports">
	{#snippet actions()}
		<Button variant="quiet" onclick={() => (adding = true)}>Add report</Button>
	{/snippet}
	{#if reports.items?.length === 0}
		<Note>No reports or alerts yet.</Note>
	{/if}
	<List
		items={reports.items?.map((report) => ({
			key: report.id,
			title: report.email,
			meta: [
				KINDS.find((item) => item.key === report.kind)?.meta(report.threshold) ??
					report.kind
			]
		})) ?? null}
		{remove}
	/>
</Section>
