<script lang="ts">
	import { untrack } from "svelte";
	import { page } from "$app/state";
	import { api } from "#lib/api.ts";
	import { collection } from "#lib/collection.svelte.ts";
	import { attempt, toast } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Meter from "#lib/ui/Meter.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Note from "#lib/ui/Note.svelte";
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
	type Export = {
		id: string;
		state: "running" | "done" | "failed";
		done: number;
		mail: boolean;
		email: boolean;
	};

	const imports = collection<{ id: number; source: string; first: string; last: string }>(
		() => `${sitePath(domain)}/imports`,
		"imports"
	);
	let format = $state("impactite");
	let source = $state<Source>("impactite");
	let file = $state<File | null>(null);
	let analytics = $state.raw<Analytics | null>(null);
	let property = $state("");
	let job = $state<Export | null>(null);
	let fetched = $state(false);

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

	// an export keeps going on the server, so coming back picks up the latest one
	$effect(() => {
		const id = page.url.searchParams.get("export");
		untrack(() => resume(id));
		return () => (job = null);
	});

	async function resume(id: string | null) {
		const found = await api<Export | null>(
			id ? `/api/app/exports/${id}` : `/api/app/exports?site_id=${encodeURIComponent(domain)}`
		).catch(() => null);
		if (found) await follow(found, false).catch(() => {});
	}

	const fetchFile = (id: string) =>
		Object.assign(document.createElement("a"), { href: `/api/app/exports/${id}/file` }).click();

	async function follow(next: Export, fetch: boolean) {
		job = next;
		fetched = false;
		while (job?.id === next.id && job.state === "running") {
			await new Promise((wait) => setTimeout(wait, 1000));
			if (job?.id !== next.id) return;
			job = await api<Export>(`/api/app/exports/${next.id}`);
		}
		if (job?.id !== next.id) return;
		if (job.state === "failed") throw new Error("Your export didn't finish. Try again.");
		if (fetch) {
			fetched = true;
			fetchFile(job.id);
		}
	}

	async function startExport() {
		const { id } = await api<{ id: string }>("/api/app/export", {
			site_id: domain,
			date_range: "all",
			format
		});
		await follow(await api<Export>(`/api/app/exports/${id}`), true);
	}

	async function emailWhenReady() {
		if (!job) return;
		const { state } = await api<{ state: Export["state"] }>(
			`/api/app/exports/${job.id}/email`,
			{}
		);
		if (job && state === "running") job.email = true;
	}

	async function loadAnalytics() {
		const was = analytics?.importing;
		analytics = await api<Analytics>(`${sitePath(domain)}/analytics`);
		property ||= analytics.properties[0]?.key ?? "";
		if (was && !analytics.importing && !analytics.error) {
			toast("Your Google Analytics history was imported.");
			await imports.load();
		}
	}

	async function importAnalytics() {
		await api(`${sitePath(domain)}/analytics`, { property });
		await loadAnalytics();
	}

	async function upload() {
		if (!file) throw new Error("Choose an export file.");
		const result = await api<{ imported: string }>(
			`${sitePath(domain)}/imports?source=${source}`,
			file
		);
		toast(`Imported ${result.imported}.`);
		file = null;
		await imports.load();
	}

	const remove = (id: string | number) =>
		attempt(() => imports.remove(id), {
			title: "Delete this import?",
			body: "The stats from this import will be deleted. You can't undo this action.",
			action: "Delete import"
		});
</script>

<Section id="data" label="data">
	<div class="flex flex-col gap-3">
		<span class="text-body text-fg">Export data</span>
		<Note
			>Download every event you've collected. Imported data isn't included in the export.</Note
		>
		<Form action="Export" submit={startExport} inline>
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
		{#if job?.state === "running"}
			<Meter value={job.done} label="Export progress" />
			<div class="flex flex-wrap items-center justify-between gap-3">
				<Note
					>{job.email
						? "You can leave this page. We'll email you when your export is ready."
						: "You can leave this page. Your export will keep going."}</Note
				>
				{#if job.mail && !job.email}
					<Button variant="quiet" onclick={emailWhenReady}
						>Email me when it's ready</Button
					>
				{/if}
			</div>
		{:else if job?.state === "done"}
			<div class="flex flex-wrap items-center justify-between gap-3">
				<Note>Your export is ready. It's available for 24 hours.</Note>
				<Button variant="quiet" onclick={() => job && fetchFile(job.id)}
					>{fetched ? "Download again" : "Download"}</Button
				>
			</div>
		{/if}
	</div>
	<span class="border-t border-line pt-4 text-body text-fg">Import data</span>
	<List
		items={imports.items?.map((item) => ({
			key: item.id,
			title: `${item.first} to ${item.last}`,
			meta: [item.source]
		})) ?? null}
		source={imports}
		empty="No imports yet."
		{remove}
	/>
	<Form action="Import" submit={upload} inline>
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
	{#if analytics?.available}
		<div class="flex flex-col gap-3 border-t border-line pt-4">
			<span class="label">google analytics 4</span>
			{#if analytics.importing}
				<Note>
					Importing from Google Analytics. This takes a few minutes. You can leave this
					page.
				</Note>
			{:else if analytics.connected && analytics.properties.length}
				<Form action="Import" submit={importAnalytics} inline>
					<Select
						value={property}
						options={analytics.properties}
						pick={(key) => (property = key)}
					/>
				</Form>
			{:else if analytics.connected}
				<Note>This Google account has no Analytics properties.</Note>
			{:else}
				<div>
					<Button
						variant="quiet"
						onclick={() => (location.href = `${sitePath(domain)}/analytics/connect`)}
						>Connect Google Analytics</Button
					>
				</div>
			{/if}
			{#if analytics.error}
				<Message tone="bad">The last import stopped: {analytics.error}</Message>
			{/if}
		</div>
	{/if}
</Section>
