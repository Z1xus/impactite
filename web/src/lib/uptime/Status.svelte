<script lang="ts">
	import Credit from "#lib/ui/Credit.svelte";
	import Empty from "#lib/ui/Empty.svelte";
	import Section from "#lib/ui/Section.svelte";
	import State from "#lib/ui/State.svelte";
	import Strip from "#lib/ui/Strip.svelte";
	import Title from "#lib/ui/Title.svelte";
	import { tick } from "./clock.svelte.ts";
	import { percent, span, stamp } from "./format.ts";
	import type { Health, Status } from "./types.ts";

	let { status }: { status: Status } = $props();

	const names = $derived(Object.fromEntries(status.monitors.map((item) => [item.id, item.name])));
	const open = $derived(status.incidents.filter((item) => item.ended_at === null));
	const past = $derived(status.incidents.filter((item) => item.ended_at !== null));
	const today = $derived(Math.floor(tick.now / 86_400) * 86_400);
	const count = (state: Health) => status.monitors.filter((item) => item.state === state).length;
	const noun = (n: number) => `${n} ${n === 1 ? "service" : "services"}`;
	const overall = $derived.by((): { state: Health; text: string } => {
		if (count("down")) return { state: "down", text: `${noun(count("down"))} down` };
		if (count("maintenance"))
			return { state: "maintenance", text: `${noun(count("maintenance"))} in maintenance` };
		if (count("slow")) return { state: "slow", text: `${noun(count("slow"))} slow` };
		if (status.monitors.every((item) => item.state === "pending"))
			return { state: "pending", text: "Waiting for the first check" };
		return { state: "up", text: "All systems up" };
	});
</script>

{#snippet incidents(list: Status["incidents"])}
	<ul class="-my-3 flex flex-col divide-y divide-line">
		{#each list as incident (incident.id)}
			<li class="flex flex-col gap-3 py-4">
				<div class="flex items-center gap-3">
					<span
						class="h-1.5 w-1.5 shrink-0 {incident.ended_at === null
							? 'bg-bad'
							: 'bg-faint'}"
					></span>
					<span class="flex min-w-0 flex-1 flex-col gap-1.5">
						<span class="truncate text-body text-fg">{names[incident.monitor]}</span>
						<span class="flex flex-wrap gap-x-3 meta">
							<span>{incident.cause}</span>
							<span>{stamp(incident.started_at)}</span>
						</span>
					</span>
					<span
						class="num text-small whitespace-nowrap {incident.ended_at === null
							? 'text-bad'
							: 'text-fg'}"
					>
						{span((incident.ended_at ?? tick.now) - incident.started_at)}
					</span>
				</div>
				{#if incident.notes.length}
					<ul class="flex flex-col gap-3 pl-4.5">
						{#each incident.notes as note (note.at)}
							<li class="flex flex-col gap-1.5">
								<span class="text-body break-words text-fg">{note.text}</span>
								<span class="meta">{stamp(note.at)}</span>
							</li>
						{/each}
					</ul>
				{/if}
			</li>
		{/each}
	</ul>
{/snippet}

<div class="flex flex-col gap-6">
	{#if status.name}<Title>{status.name}</Title>{/if}
	<div
		class="flex items-center gap-3 rounded-px border border-line bg-panel px-5 py-4 text-body text-fg"
	>
		<State state={overall.state} dot />
		{overall.text}
	</div>

	{#if open.length}
		<Section label="open incidents">{@render incidents(open)}</Section>
	{/if}

	{#if status.maintenance.length}
		<Section label="maintenance">
			<ul class="-my-3 flex flex-col divide-y divide-line">
				{#each status.maintenance as window (window.id)}
					<li class="flex items-center gap-3 py-4">
						<span class="flex min-w-0 flex-1 flex-col gap-1.5">
							<span class="text-body break-words text-fg"
								>{window.note || "Maintenance"}</span
							>
							<span class="flex flex-wrap gap-x-3 meta">
								<span>{stamp(window.starts_at)} to {stamp(window.ends_at)}</span>
								<span>{window.monitors.map((id) => names[id]).join(", ")}</span>
							</span>
						</span>
						<span class="label whitespace-nowrap">
							{window.starts_at <= tick.now ? "now" : "upcoming"}
						</span>
					</li>
				{/each}
			</ul>
		</Section>
	{/if}

	<Section label="services">
		<ul class="-my-3 flex flex-col divide-y divide-line">
			{#each status.monitors as monitor (monitor.id)}
				<li class="flex flex-col gap-3 py-4">
					<div class="flex items-center justify-between gap-4">
						<span class="min-w-0 truncate text-body text-fg">{monitor.name}</span>
						<span class="flex items-center gap-4">
							<span class="num text-small text-fg">
								{monitor.uptime === null ? "–" : percent(monitor.uptime)}
							</span>
							<State state={monitor.state} since={monitor.since} />
						</span>
					</div>
					<Strip
						cells={monitor.strip}
						{today}
						paused={monitor.state === "paused"}
						stretch
					/>
				</li>
			{/each}
		</ul>
	</Section>

	<Section label="past incidents">
		{#if past.length}
			{@render incidents(past)}
		{:else}
			<Empty compact text="No incidents in the last 30 days." />
		{/if}
	</Section>

	{#if status.look.brand}
		<Credit label="Status" />
	{/if}
</div>
