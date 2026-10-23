<script lang="ts">
	import { api } from "#lib/api.ts";
	import { toast } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import List from "#lib/ui/List.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";
	import Upload from "#lib/ui/Upload.svelte";
	import type { Imported } from "./types.ts";

	let { open = $bindable(false), team }: { open: boolean; team: number } = $props();

	let file = $state<File | null>(null);
	let preview = $state.raw<Imported | null>(null);
	let error = $state("");
	let busy = $state(false);

	const send = (dry: boolean) =>
		api<Imported>(`/api/app/uptime/import?team=${team}${dry ? "&dry=1" : ""}`, file);

	$effect(() => {
		if (!file) return;
		preview = null;
		error = "";
		let stale = false;
		send(true)
			.then((result) => !stale && (preview = result))
			.catch(
				(err) =>
					!stale && (error = err instanceof Error ? err.message : "Couldn't read it.")
			);
		return () => (stale = true);
	});

	$effect(() => {
		if (open) return;
		file = null;
		preview = null;
		error = "";
	});

	const plural = (count: number, one: string, many: string) =>
		`${count} ${count === 1 ? one : many}`;

	const counts = $derived(
		preview
			? (
					[
						[preview.monitors, "monitor", "monitors"],
						[preview.existing, "monitor already here", "monitors already here"],
						[preview.incidents, "incident", "incidents"],
						[preview.alerts, "alert", "alerts"],
						[preview.maintenance, "maintenance window", "maintenance windows"],
						[preview.pages, "status page", "status pages"]
					] as const
				)
					.filter(([count]) => count)
					.map(([count, one, many]) => ({ key: one, title: plural(count, one, many) }))
			: []
	);
	const left = $derived(preview ? [...preview.noted, ...preview.skipped] : []);

	async function run() {
		busy = true;
		try {
			const done = await send(false);
			toast(`Imported ${plural(done.monitors, "monitor", "monitors")}.`);
			open = false;
		} catch (err) {
			error = err instanceof Error ? err.message : "Something went wrong.";
		} finally {
			busy = false;
		}
	}
</script>

<Dialog title="import from uptime kuma" bind:open>
	<div class="flex flex-col gap-4">
		<Upload
			bind:file
			accept=".db,.sqlite,.sqlite3"
			zone={{ title: "Drop kuma.db here", meta: "/app/data/kuma.db in the Kuma container" }}
		/>
		{#if file && !preview && !error}
			<Skeleton class="h-24 w-full" />
		{:else if preview}
			<List items={counts} empty="Nothing new to import." />
			{#if left.length}
				<List
					items={left.map((item, i) => ({
						key: i,
						title: item.name,
						meta: [item.reason]
					}))}
				/>
			{/if}
		{/if}
		<Message tone="bad">{error}</Message>
		{#if preview?.monitors}
			<Button class="self-end" {busy} onclick={run}
				>Import {plural(preview.monitors, "monitor", "monitors")}</Button
			>
		{/if}
	</div>
</Dialog>
