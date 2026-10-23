<script lang="ts">
	import { untrack } from "svelte";
	import { api, get } from "#lib/api.ts";
	import Checks from "#lib/ui/Checks.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import type { Monitor } from "./types.ts";

	let {
		open = $bindable(false),
		team,
		preset = [],
		saved
	}: { open: boolean; team: number; preset?: number[]; saved?: () => unknown } = $props();

	const local = (seconds: number) => {
		const time = new Date(seconds * 1000 - new Date().getTimezoneOffset() * 60_000);
		return time.toISOString().slice(0, 16);
	};

	let monitors = $state.raw<{ key: string; label: string }[]>([]);
	let start = $state("");
	let end = $state("");
	let note = $state("");
	let chosen = $state<string[]>([]);

	$effect(() => {
		if (!open) return;
		untrack(() => {
			const now = Math.ceil(Date.now() / 1000 / 900) * 900;
			start = local(now);
			end = local(now + 3600);
			note = "";
			chosen = preset.map(String);
		});
		get<{ monitors: Monitor[] }>(`/api/app/uptime?team=${team}`).then((reply) => {
			monitors = reply.monitors.map((item) => ({ key: String(item.id), label: item.name }));
		});
	});

	async function submit() {
		await api("/api/app/uptime/maintenance", {
			team,
			starts_at: Math.floor(new Date(start).getTime() / 1000),
			ends_at: Math.floor(new Date(end).getTime() / 1000),
			note,
			monitors: chosen.map(Number)
		});
		open = false;
		await saved?.();
	}
</script>

<Dialog title="plan maintenance" bind:open>
	<Form action="Plan maintenance" done="Planned." {submit}>
		<div class="grid gap-4 sm:grid-cols-2">
			<Field label="start" type="datetime-local" bind:value={start} required />
			<Field label="end" type="datetime-local" bind:value={end} required />
		</div>
		<Field
			label="what is happening"
			placeholder="Database upgrade"
			maxlength={200}
			bind:value={note}
		/>
		<Checks bind:value={chosen} options={monitors} data />
	</Form>
</Dialog>
