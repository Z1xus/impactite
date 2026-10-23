<script lang="ts">
	import { untrack } from "svelte";
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { resolve } from "$app/paths";
	import more from "pixelarticons/svg/more-vertical.svg?raw";
	import { api, ApiError, get } from "#lib/api.ts";
	import Chart from "#lib/dither/Chart.svelte";
	import type { Band } from "#lib/dither/chart.ts";
	import { session } from "#lib/session.svelte.ts";
	import { attempt, toast } from "#lib/toast.svelte.ts";
	import Back from "#lib/ui/Back.svelte";
	import Code from "#lib/ui/Code.svelte";
	import Empty from "#lib/ui/Empty.svelte";
	import Glyph from "#lib/ui/Glyph.svelte";
	import Menu from "#lib/ui/Menu.svelte";
	import Missing from "#lib/ui/Missing.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";
	import State from "#lib/ui/State.svelte";
	import Strip from "#lib/ui/Strip.svelte";
	import Title from "#lib/ui/Title.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import { tick } from "#lib/uptime/clock.svelte.ts";
	import Figures from "#lib/uptime/Figures.svelte";
	import { every, latency, span, stamp } from "#lib/uptime/format.ts";
	import { glyph } from "#lib/uptime/glyph.ts";
	import Incidents from "#lib/uptime/Incidents.svelte";
	import { stream } from "#lib/uptime/live.svelte.ts";
	import MaintenanceDialog from "#lib/uptime/MaintenanceDialog.svelte";
	import MonitorDialog from "#lib/uptime/MonitorDialog.svelte";
	import { writes, type Check, type Detail, type Range } from "#lib/uptime/types.ts";

	const id = $derived(page.params.id ?? "");
	const RANGES = [
		{ key: "24h", label: "24 h" },
		{ key: "7d", label: "7 d" },
		{ key: "30d", label: "30 d" },
		{ key: "90d", label: "90 d" }
	] satisfies { key: Range; label: string }[];

	let range = $state<Range>("24h");
	let detail = $state.raw<Detail | null>(null);
	let missing = $state(false);
	let failed = $state(false);
	let editing = $state(false);
	let scheduling = $state(false);
	let gone = false;
	let timer: ReturnType<typeof setTimeout> | undefined;

	const monitor = $derived(detail?.monitor);
	const editable = $derived(writes(detail?.role));
	const beat = $derived(monitor?.kind === "heartbeat");
	const url = $derived(`/api/app/uptime/${id}`);
	const pinged = $derived(!!detail?.series.some((point) => point.ms));

	async function load(fresh = false) {
		if (gone) return;
		const path = `${url}?range=${range}`;
		try {
			const next = await (fresh ? api : get)<Detail>(path);
			if (path !== `${url}?range=${range}`) return;
			detail = next;
			failed = false;
		} catch (err) {
			if (err instanceof ApiError && err.status === 404) missing = true;
			else if (!detail) failed = true;
		}
	}

	$effect(() => {
		void id;
		void range;
		untrack(() => {
			if (!detail || detail.monitor.id !== Number(id)) detail = null;
			load();
		});
		const refresh = setInterval(() => load(true), 60_000);
		return () => {
			clearInterval(refresh);
			clearTimeout(timer);
		};
	});

	stream(
		() => session.team,
		(event: Check) => {
			if (!detail || event.id !== detail.monitor.id) return;
			const known = detail.monitor;
			if (known.state === event.state && known.at === event.at) return;
			detail = { ...detail, monitor: { ...known, ...event } };
			clearTimeout(timer);
			timer = setTimeout(() => load(true), 600);
		}
	);

	const labels = $derived(detail?.series.map((point) => stamp(point.t)) ?? []);
	const values = $derived.by(() => {
		let last = 0;
		return (detail?.series ?? []).map((point) => (last = point.ms || last));
	});
	const bands = $derived.by((): Band[] => {
		const series = detail?.series ?? [];
		return (detail?.incidents ?? []).flatMap((incident) => {
			const from = Math.max(
				0,
				series.findLastIndex((point) => point.t <= incident.started_at)
			);
			const to = series.findLastIndex((point) => point.t <= (incident.ended_at ?? Infinity));
			if (to < 0) return [];
			const label = `${incident.cause}, ${incident.ended_at ? span(incident.duration) : "ongoing"}`;
			return [{ from, to: Math.max(from, to), label }];
		});
	});

	const act = (path: string, done?: string) =>
		attempt(async () => {
			await api(`${url}/${path}`, {});
			if (done) toast(done);
			await load(true);
		});

	const check = () =>
		attempt(async () => {
			const result = await api<{ ok: boolean; cause: string | null }>(`${url}/check`, {});
			toast(result.ok ? "Checked." : (result.cause ?? "Down."), !result.ok);
			await load(true);
		});

	const remove = () =>
		attempt(
			async () => {
				await api(url, undefined, "DELETE");
				gone = true;
				await goto(resolve("/uptime"));
			},
			{
				title: "Delete this monitor?",
				body: "Its history is removed.",
				action: "Delete monitor"
			}
		);

	const actions = $derived([
		...(beat ? [] : [{ label: "Check now", onclick: check }]),
		{
			label: monitor?.paused ? "Resume" : "Pause",
			onclick: () => act(monitor?.paused ? "resume" : "pause")
		},
		{ label: "Plan maintenance", onclick: () => (scheduling = true) },
		{
			label: monitor?.muted ? "Unmute alerts" : "Mute alerts",
			onclick: () =>
				attempt(async () => {
					await api(url, { muted: !monitor?.muted }, "PATCH");
					await load(true);
				})
		},
		"divider" as const,
		{ label: "Edit", onclick: () => (editing = true) },
		{ label: "Delete", onclick: remove }
	]);
</script>

<svelte:head>
	<title>{monitor?.name ?? "Monitor"}</title>
</svelte:head>

{#if monitor && session.team}
	<MonitorDialog bind:open={editing} team={session.team} {monitor} saved={() => load(true)} />
	<MaintenanceDialog
		bind:open={scheduling}
		team={session.team}
		preset={[monitor.id]}
		saved={() => load(true)}
	/>
{/if}

<Shell>
	{#if missing}
		<Missing text="This monitor doesn't exist." />
	{:else if failed}
		<Empty retry={() => load(true)} />
	{:else}
		<div class="flex flex-col gap-6">
			<div class="flex min-h-control items-center justify-between gap-4">
				<Title>
					<Back href={resolve("/uptime")} title="Back to uptime">
						{#if monitor}
							<Glyph spec={glyph(monitor)} fallback={monitor.name} />
							<span class="truncate">{monitor.name}</span>
						{:else}
							<Skeleton class="h-5 w-40" />
						{/if}
					</Back>
					{#if monitor}
						<State state={monitor.state} since={monitor.since} />
					{/if}
				</Title>
				{#if editable}
					<Menu
						label="Monitor options"
						icon={more}
						plain
						iconOnly
						align="right"
						items={actions}
					/>
				{/if}
			</div>

			{#if monitor}
				<div class="flex flex-wrap items-center gap-x-5 gap-y-3 meta leading-4">
					{#if monitor.target && monitor.kind === "http"}
						<!-- eslint-disable svelte/no-navigation-without-resolve -- the monitored site -->
						<a
							href={monitor.target}
							target="_blank"
							rel="noreferrer"
							class="max-w-full truncate hover:text-fg">{monitor.target}</a
						>
						<!-- eslint-enable svelte/no-navigation-without-resolve -->
					{:else if monitor.target && !beat}
						<span class="max-w-full truncate">{monitor.target}</span>
					{/if}
					<span>{beat ? "runs" : "checked"} every {every(monitor.interval)}</span>
					{#if monitor.state !== "pending"}
						<span
							>{beat ? "last ping" : "last check"}
							{span(tick.now - monitor.at)} ago</span
						>
					{/if}
					{#if monitor.muted}
						<span class="text-warn">alerts muted</span>
					{/if}
					{#if !(beat && !pinged)}
						<div class="ml-auto">
							<Toggle bind:value={range} options={RANGES} />
						</div>
					{/if}
				</div>
			{:else}
				<div class="flex min-h-control items-center"><Skeleton class="h-3 w-72" /></div>
			{/if}

			<section class="rounded-px border border-line bg-panel">
				<Figures {detail} />
				<div class="flex flex-col gap-4 px-4 pt-3 pb-5">
					{#if beat && !pinged}
						<div class="flex flex-col gap-3 pt-2">
							{#if monitor?.target}
								<Code code={monitor.target} />
								<Code code="curl -fsS -m 10 --retry 3 {monitor.target}" />
							{/if}
						</div>
					{:else}
						{#if !detail}
							<Skeleton class="h-[200px] w-full" />
						{:else if values.some(Boolean)}
							<Chart {values} {labels} {bands} format={latency} height={200} />
						{:else}
							<Empty
								compact
								text={detail.series.length
									? "No responses in this range."
									: "No checks yet."}
							/>
						{/if}
					{/if}
				</div>
				<div class="flex flex-col gap-3 border-t border-line px-4 py-4">
					<span class="label">90 days</span>
					{#if monitor}
						<Strip
							cells={monitor.strip}
							today={Math.floor(tick.now / 86_400) * 86_400}
							paused={monitor.paused}
							stretch
						/>
					{:else}
						<Skeleton class="h-[22px] w-full" />
					{/if}
				</div>
			</section>

			<Section label="incidents">
				{#if detail}
					<Incidents incidents={detail.incidents} {editable} reload={() => load(true)} />
				{:else}
					<Skeleton class="h-14 w-full" />
				{/if}
			</Section>
		</div>
	{/if}
</Shell>
