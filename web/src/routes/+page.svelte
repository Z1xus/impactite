<script lang="ts">
	import { untrack } from "svelte";
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api } from "#lib/api.ts";
	import { icon } from "#lib/icons/resolve.ts";
	import { display } from "#lib/display.svelte.ts";
	import { currentTeam, refresh, teamTimezone } from "#lib/session.svelte.ts";
	import { sparks } from "#lib/sparks.ts";
	import { swap } from "#lib/swap.ts";
	import { watching } from "#lib/uptime/live.svelte.ts";
	import type { Team } from "#lib/api.ts";
	import { warm } from "#lib/dashboard/warm.ts";
	import Button from "#lib/ui/Button.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Empty from "#lib/ui/Empty.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import SiteCard from "#lib/ui/SiteCard.svelte";
	import Title from "#lib/ui/Title.svelte";
	import { ZONES } from "#lib/zones.ts";

	const current = $derived(currentTeam());
	const snapshot = $derived(JSON.stringify(current));
	let shown = $state.raw<{
		team: Team;
		series: Record<string, number[]>;
		failed: boolean;
	} | null>(null);
	let tries = $state(0);

	const monitors = watching(() => current?.id ?? null);
	const watched = $derived(
		new Map(monitors.monitors?.flatMap((item) => (item.site ? [[item.site, item]] : [])))
	);

	const team = $derived(shown?.team ?? current);
	const editable = $derived(["owner", "admin", "editor"].includes(team?.role ?? ""));

	$effect(() => {
		void snapshot;
		void tries;
		const next = untrack(() => current);
		if (!next) return;
		let stale = false;
		sparks(next).then((result) => {
			if (stale) return;
			if (shown) swap(() => (shown = { team: next, ...result }));
			else shown = { team: next, ...result };
		});
		return () => (stale = true);
	});

	let domain = $state("");
	let adding = $state(false);
	let timezone = $state(Intl.DateTimeFormat().resolvedOptions().timeZone);

	async function add() {
		const site = await api<{ domain: string }>("/api/app/sites", {
			team: team?.id,
			domain,
			timezone
		});
		domain = "";
		adding = false;
		await refresh();
		await goto(resolve("/[site]", { site: site.domain }));
	}
</script>

<svelte:head>
	<title>Sites</title>
</svelte:head>

<Dialog title="add site" bind:open={adding}>
	<Form action="Add site" submit={add}>
		<Field label="domain" placeholder="example.com" bind:value={domain} required />
		<Select
			label="timezone"
			value={timezone}
			options={ZONES}
			data
			pick={(zone) => (timezone = zone)}
		/>
	</Form>
</Dialog>

<Shell>
	<div class="flex flex-col gap-6">
		<div class="flex min-h-control items-center justify-between gap-4">
			<Title>
				{#if team}
					{team.personal ? "Personal" : team.name}
				{:else}
					<Skeleton class="h-6 w-40" />
				{/if}
			</Title>
			{#if editable && team?.sites.length}
				<Button onclick={() => (adding = true)}>Add site</Button>
			{/if}
		</div>
		{#if shown?.failed}
			<Empty compact retry={() => tries++} />
		{/if}
		{#if !team || team.sites.length}
			<ul class="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
				{#if !team}
					{#each [0, 1, 2] as placeholder (placeholder)}
						<li><SiteCard /></li>
					{/each}
				{:else}
					{#if team.sites.length > 1 && display.all === "show"}
						<li>
							<SiteCard
								href={resolve("/all")}
								title="All sites"
								values={shown?.series.all}
								warm={() => warm(`team:${team.id}`, teamTimezone(team))}
							/>
						</li>
					{/if}
					{#each team.sites as site (site.id)}
						<li>
							<SiteCard
								href={resolve("/[site]", { site: site.domain })}
								title={site.domain}
								glyph={icon("event:hostname", site.domain)}
								values={shown?.series[site.domain]}
								monitor={watched.get(site.domain)}
								warm={() => warm(site.domain, site.timezone)}
							/>
						</li>
					{/each}
				{/if}
			</ul>
		{:else}
			<Empty text="No sites yet.">
				{#if editable}
					<Button onclick={() => (adding = true)}>Add site</Button>
				{/if}
			</Empty>
		{/if}
	</div>
</Shell>
