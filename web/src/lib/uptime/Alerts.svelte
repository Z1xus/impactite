<script lang="ts">
	import { api } from "#lib/api.ts";
	import { channel, detect } from "#lib/channels.ts";
	import { collection } from "#lib/collection.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Target from "#lib/ui/Target.svelte";
	import { span } from "./format.ts";
	import type { Alert } from "./types.ts";

	let { team, editable }: { team: number; editable: boolean } = $props();

	const alerts = collection<Alert>(
		() => "/api/app/uptime/alerts",
		"alerts",
		() => `?team=${team}`
	);
	let target = $state("");
	let testing = $state<number | null>(null);
	let now = $state(Date.now() / 1000);

	$effect(() => {
		alerts.load();
		const timer = setInterval(() => (now = Date.now() / 1000), 30_000);
		return () => clearInterval(timer);
	});

	async function add() {
		const { error } = detect(target);
		if (error) throw new Error(`${error}.`);
		await alerts.add({ team, target });
		target = "";
	}

	async function test(id: number) {
		testing = id;
		try {
			alerts.keep(
				await api<{ alerts: Alert[] }>(`/api/app/uptime/alerts/${id}/test?team=${team}`, {})
			);
		} finally {
			testing = null;
			now = Date.now() / 1000;
		}
	}

	const remove = (id: string | number) =>
		attempt(() => alerts.remove(id), {
			title: "Delete this alert?",
			body: "You'll no longer get alerts here.",
			action: "Delete alert"
		});

	function status(alert: Alert) {
		if (alert.kind === "email" && alerts.extra?.mail === false)
			return { text: "Mail is not set up", tone: "warn" as const };
		if (alert.failure) return { text: `Failed, ${alert.failure}`, tone: "bad" as const };
		if (alert.sent_at) return { text: `Sent ${span(now - alert.sent_at)} ago` };
		return { text: "Not sent yet" };
	}
</script>

<Section id="alerts" label="alerts">
	{#if editable}
		<div class="border-b border-line pb-5">
			<Form inline action="Add" submit={add}>
				<Target label="send to" bind:value={target} mail={alerts.extra?.mail !== false} />
			</Form>
		</div>
	{/if}
	<List
		items={alerts.items?.map((alert) => ({
			key: alert.id,
			title: channel(alert.kind)?.name ?? alert.kind,
			glyph: channel(alert.kind)?.spec,
			meta: [alert.label],
			status: status(alert)
		})) ?? null}
		source={alerts}
		empty={editable ? undefined : "No alerts yet."}
		remove={editable ? remove : undefined}
	>
		{#snippet control(key)}
			{#if editable}
				<Button
					variant="quiet"
					size="sm"
					busy={testing === Number(key)}
					onclick={() => test(Number(key))}>Test</Button
				>
			{/if}
		{/snippet}
	</List>
</Section>
