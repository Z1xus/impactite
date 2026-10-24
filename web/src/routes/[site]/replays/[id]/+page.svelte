<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { api } from "#lib/api.ts";
	import Loading from "#lib/dashboard/Loading.svelte";
	import { icon } from "#lib/icons/resolve.ts";
	import Player from "#lib/replay/Player.svelte";
	import type { Replay } from "#lib/replay/types.ts";
	import { findSite, session } from "#lib/session.svelte.ts";
	import { display } from "#lib/stats.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Back from "#lib/ui/Back.svelte";
	import Button from "#lib/ui/Button.svelte";
	import Empty from "#lib/ui/Empty.svelte";
	import Glyph from "#lib/ui/Glyph.svelte";
	import Missing from "#lib/ui/Missing.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";
	import Title from "#lib/ui/Title.svelte";
	import { stamp } from "#lib/uptime/format.ts";

	const domain = $derived(page.params.site ?? "");
	const id = $derived(page.params.id ?? "");
	const site = $derived(findSite(domain));
	const editable = $derived(["owner", "admin", "editor"].includes(site?.role ?? ""));
	const list = $derived(resolve("/[site]/replays", { site: domain }));

	let found = $state.raw<{
		replay: Replay;
		events: Parameters<typeof Player>[1]["events"];
	} | null>(null);
	let failed = $state("");

	$effect(() => {
		found = null;
		failed = "";
		api<NonNullable<typeof found>>(`/api/app/sites/${domain}/replays/${id}`).then(
			(result) => (found = result),
			(err) => (failed = err instanceof Error ? err.message : "Couldn't load.")
		);
	});

	const remove = () =>
		attempt(
			async () => {
				await api(`/api/app/sites/${domain}/replays/${id}`, undefined, "DELETE");
				await goto(list);
			},
			{
				title: "Delete this replay?",
				body: "You can't undo this action.",
				action: "Delete replay"
			}
		);
</script>

<svelte:head>
	<title>{domain} replay</title>
</svelte:head>

<Shell>
	{#if site}
		<div class="flex flex-col gap-6">
			<div class="flex flex-wrap items-center gap-3">
				<Title>
					<Back href={list} title="Back to replays">
						{#if found?.replay.country}
							<Glyph spec={icon("visit:country", found.replay.country)} />
						{/if}
						{found?.replay.entry ?? "replay"}
					</Back>
				</Title>
				{#if found}
					<span class="flex flex-wrap gap-x-4 gap-y-1 meta">
						<span>{stamp(found.replay.started_at / 1000)}</span>
						<span
							>{[found.replay.browser, found.replay.os]
								.filter(Boolean)
								.join(" on ")}</span
						>
						<span>{display("visit:country", found.replay.country)}</span>
					</span>
				{/if}
				{#if editable}
					<Button variant="quiet" class="ml-auto" onclick={remove}>Delete</Button>
				{/if}
			</div>
			{#if failed}
				<section class="rounded-px border border-line bg-panel">
					<Empty text={failed} />
				</section>
			{:else if found}
				<Player {domain} events={found.events} />
			{:else}
				<Skeleton class="h-[60vh] w-full" />
			{/if}
		</div>
	{:else if session.me}
		<Missing />
	{:else}
		<Loading />
	{/if}
</Shell>
