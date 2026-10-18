<script lang="ts">
	import { untrack } from "svelte";
	import { api } from "#lib/api.ts";
	import { collection } from "#lib/collection.svelte.ts";
	import { archive } from "#lib/download.ts";
	import { attempt, toast } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import Upload from "#lib/ui/Upload.svelte";
	import { sitePath } from "./site.ts";

	let { domain }: { domain: string } = $props();

	type Analytics = {
		available: boolean;
		connected: boolean;
		properties: { key: string; label: string }[];
		importing: boolean;
		error: string | null;
	};
	type Source = "impactite" | "plausible" | "goatcounter" | "umami";

	const imports = collection<{ id: number; source: string; first: string; last: string }>(
		() => `${sitePath(domain)}/imports`,
		"imports"
	);
	let format = $state("impactite");
	let source = $state<Source>("impactite");
	let file = $state<File | null>(null);
	let imported = $state("");
	let analytics = $state.raw<Analytics | null>(null);
	let property = $state("");

	const accept = $derived(
		source === "goatcounter" ? ".csv,.gz" : source === "umami" ? ".csv" : ".zip"
	);

	$effect(() => {
		imports.load();
		untrack(loadAnalytics);
	});

	// an import runs on the server, so this checks back until it's done
	$effect(() => {
		if (!analytics?.importing) return;
		const timer = setInterval(loadAnalytics, 4000);
		return () => clearInterval(timer);
	});

	async function loadAnalytics() {
		const was = analytics?.importing;
		analytics = await api<Analytics>(`${sitePath(domain)}/analytics`);
		property ||= analytics.properties[0]?.key ?? "";
		if (was && !analytics.importing && !analytics.error) {
			toast("Google Analytics history imported.");
			await imports.load();
		}
	}

	async function importAnalytics() {
		await api(`${sitePath(domain)}/analytics`, { property });
		await loadAnalytics();
	}

	async function upload() {
		if (!file) throw new Error("Choose an export file.");
		imported = "";
		const result = await api<{ imported: string }>(
			`${sitePath(domain)}/imports?source=${source}`,
			file
		);
		imported = `Imported ${result.imported}.`;
		file = null;
		await imports.load();
	}

	const remove = (id: string | number) =>
		attempt(() => imports.remove(id), {
			title: "Remove this import?",
			body: "Its imported stats are deleted.",
			action: "remove import"
		});
</script>

<Section id="imports" label="data">
	<div class="flex flex-col gap-3">
		<span class="text-body text-fg">Export data</span>
		<Message>
			Export collected events from all dates. Imported summaries are not included.
		</Message>
		<Form action="download" submit={() => archive(domain, format)} inline>
			<Select
				label="export format"
				value={format}
				pick={(value) => (format = value)}
				data
				options={[
					{ key: "impactite", label: "Impactite archive" },
					{ key: "plausible", label: "Plausible CSV archive" },
					{ key: "goatcounter", label: "GoatCounter CSV" }
				]}
			/>
		</Form>
	</div>
	<span class="border-t border-line pt-4 text-body text-fg">Import data</span>
	<List
		items={imports.items?.map((item) => ({
			key: item.id,
			title: `${item.first} to ${item.last}`,
			meta: [item.source]
		})) ?? null}
		{remove}
	/>
	<Form action="import" submit={upload} inline>
		<Upload bind:file {accept} />
		<Toggle
			bind:value={source}
			options={[
				{ key: "impactite", label: "Impactite" },
				{ key: "plausible", label: "Plausible" },
				{ key: "goatcounter", label: "GoatCounter" },
				{ key: "umami", label: "Umami" }
			]}
		/>
	</Form>
	<Message tone="cyan">{imported}</Message>
	{#if analytics?.available}
		<div class="flex flex-col gap-3 border-t border-line pt-4">
			<span class="label">google analytics 4</span>
			{#if analytics.importing}
				<Message>
					Importing from Google Analytics. This takes a few minutes. You can leave this
					page.
				</Message>
			{:else if analytics.connected && analytics.properties.length}
				<Form action="import" submit={importAnalytics} inline>
					<Select
						value={property}
						options={analytics.properties}
						pick={(key) => (property = key)}
					/>
				</Form>
			{:else if analytics.connected}
				<Message>This Google account has no Analytics properties.</Message>
			{:else}
				<div>
					<Button
						variant="quiet"
						onclick={() => (location.href = `${sitePath(domain)}/analytics/connect`)}
						>Connect google analytics</Button
					>
				</div>
			{/if}
			{#if analytics.error}
				<Message tone="bad">The last import stopped: {analytics.error}</Message>
			{/if}
		</div>
	{/if}
</Section>
