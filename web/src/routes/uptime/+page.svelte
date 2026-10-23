<script lang="ts">
	import { resolve } from "$app/paths";
	import bell from "pixelarticons/svg/bell.svg?raw";
	import more from "pixelarticons/svg/more-vertical.svg?raw";
	import { get } from "#lib/api.ts";
	import { channel } from "#lib/channels.ts";
	import { currentTeam, session } from "#lib/session.svelte.ts";
	import ImportDialog from "#lib/uptime/ImportDialog.svelte";
	import { order, watching } from "#lib/uptime/live.svelte.ts";
	import MonitorDialog from "#lib/uptime/MonitorDialog.svelte";
	import Tabs from "#lib/uptime/Tabs.svelte";
	import { writes, type Alert, type Health } from "#lib/uptime/types.ts";
	import Button from "#lib/ui/Button.svelte";
	import Empty from "#lib/ui/Empty.svelte";
	import Glyph from "#lib/ui/Glyph.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Menu from "#lib/ui/Menu.svelte";
	import MonitorRow from "#lib/ui/MonitorRow.svelte";
	import State from "#lib/ui/State.svelte";

	const team = $derived(currentTeam());
	const live = watching(() => session.team);
	const editable = $derived(writes(live.role || team?.role));
	const monitors = $derived(live.monitors && order(live.monitors));
	const counts = $derived(
		(["down", "slow", "up", "maintenance", "paused", "pending"] as Health[])
			.map((state) => ({
				state,
				count: monitors?.filter((item) => item.state === state).length ?? 0
			}))
			.filter((item) => item.count)
	);

	let adding = $state(false);
	let importing = $state(false);
	let alerts = $state.raw<Alert[] | null>(null);

	$effect(() => {
		if (!team || !editable) return;
		get<{ alerts: Alert[] }>(`/api/app/uptime/alerts?team=${team.id}`)
			.then((reply) => (alerts = reply.alerts))
			.catch(() => (alerts = null));
	});

	const kinds = $derived([...new Set(alerts?.map((alert) => alert.kind))]);
</script>

{#if team}
	<MonitorDialog bind:open={adding} team={team.id} />
	<ImportDialog bind:open={importing} team={team.id} />
{/if}

<Tabs>
	{#snippet actions()}
		{#if editable && monitors?.length}
			<div class="flex items-center gap-2">
				<Button onclick={() => (adding = true)}>Add monitor</Button>
				<Menu
					label="More"
					icon={more}
					iconOnly
					align="right"
					items={[
						{ label: "Import from Uptime Kuma", onclick: () => (importing = true) }
					]}
				/>
			</div>
		{/if}
	{/snippet}
	{#if live.failed}
		<Empty compact retry={live.load} />
	{:else if monitors && !monitors.length}
		<Empty text="No monitors yet.">
			{#if editable}
				<div class="flex gap-2">
					<Button onclick={() => (adding = true)}>Add monitor</Button>
					<Button variant="quiet" onclick={() => (importing = true)}
						>Import from Uptime Kuma</Button
					>
				</div>
			{/if}
		</Empty>
	{:else}
		<div class="flex flex-wrap items-center gap-x-5 gap-y-3">
			{#each counts as item (item.state)}
				<span class="flex items-center gap-2">
					<State state={item.state} dot />
					<span class="font-mono text-small text-fg">{item.count}</span>
					<span class="label">{item.state}</span>
				</span>
			{/each}
			{#if editable && alerts}
				<a
					href={resolve("/uptime/alerts")}
					class="-my-2 ml-auto flex h-control items-center gap-2 text-small {kinds.length
						? 'text-dim hover:text-fg'
						: 'text-warn hover:text-fg'}"
					title={kinds.length
						? `Alerts go to ${kinds.map((kind) => channel(kind)?.name).join(", ")}`
						: "Nobody is told when a monitor goes down"}
				>
					{#if kinds.length}
						<span class="flex gap-1">
							{#each kinds.slice(0, 3) as kind (kind)}
								{@const spec = channel(kind)?.spec}
								{#if spec}<Glyph {spec} />{/if}
							{/each}
						</span>
						{alerts.length === 1 ? "1 alert" : `${alerts.length} alerts`}
					{:else}
						<Icon svg={bell} size={12} />No alerts
					{/if}
				</a>
			{/if}
		</div>
		<section class="rounded-px border border-line bg-panel">
			<MonitorRow head />
			<ul class="flex flex-col divide-y divide-line md:border-t md:border-line">
				{#if !monitors}
					{#each [0, 1, 2] as placeholder (placeholder)}
						<li><MonitorRow /></li>
					{/each}
				{:else}
					{#each monitors as monitor (monitor.id)}
						<li>
							<MonitorRow
								{monitor}
								today={live.today}
								href={resolve("/uptime/[id]", { id: String(monitor.id) })}
							/>
						</li>
					{/each}
				{/if}
			</ul>
		</section>
	{/if}
</Tabs>
