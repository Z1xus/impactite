<script lang="ts">
	import { resolve } from "$app/paths";
	import { api } from "#lib/api.ts";
	import Sections from "#lib/dashboard/Sections.svelte";
	import SitePicker from "#lib/dashboard/SitePicker.svelte";
	import Unrecorded from "#lib/dashboard/Unrecorded.svelte";
	import { icon } from "#lib/icons/resolve.ts";
	import { findSite } from "#lib/session.svelte.ts";
	import { saveSite } from "#lib/settings/site.ts";
	import { display } from "#lib/stats.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Empty from "#lib/ui/Empty.svelte";
	import List from "#lib/ui/List.svelte";
	import { span, stamp } from "#lib/uptime/format.ts";
	import type { Replay } from "./types.ts";

	let { domain }: { domain: string } = $props();

	const PAGE = 50;

	const on = $derived(!findSite(domain)?.skip.includes("replays"));
	const editable = $derived(["owner", "admin", "editor"].includes(findSite(domain)?.role ?? ""));

	let replays = $state.raw<Replay[] | null>(null);
	let more = $state(false);
	let failed = $state(false);
	let tries = $state(0);

	const path = $derived(`/api/app/sites/${domain}/replays`);

	async function load(before?: number) {
		failed = false;
		try {
			const next = (
				await api<{ replays: Replay[] }>(before ? `${path}?before=${before}` : path)
			).replays;
			replays = before ? [...(replays ?? []), ...next] : next;
			more = next.length === PAGE;
		} catch {
			failed = true;
		}
	}

	$effect(() => {
		void tries;
		if (on) void load();
	});

	const remove = (key: string) =>
		attempt(
			async () => {
				await api(`${path}/${key}`, undefined, "DELETE");
				replays = replays?.filter((item) => item.key !== key) ?? null;
			},
			{
				title: "Delete this replay?",
				body: "You can't undo this action.",
				action: "Delete replay"
			}
		);
</script>

<div class="flex flex-col gap-6">
	<div class="flex flex-wrap items-center gap-3">
		<SitePicker {domain} section="replays" />
		<Sections {domain} />
	</div>

	<section class="rounded-px border border-line bg-panel px-4 py-3">
		{#if !on}
			<Empty text="Replays are off. Turn them on to watch how visitors use your site.">
				{#if editable}
					<Button
						variant="quiet"
						onclick={() =>
							saveSite(domain, {
								skip:
									findSite(domain)?.skip.filter((key) => key !== "replays") ?? []
							})}>Turn on</Button
					>
				{/if}
			</Empty>
		{:else if failed && !replays}
			<Empty retry={() => tries++} />
		{:else if replays?.length === 0}
			<Unrecorded {domain} text="No replays yet." compact />
		{:else}
			<div class="py-3">
				<List
					items={replays?.map((item) => ({
						key: item.key,
						title: item.entry || "/",
						href: resolve("/[site]/replays/[id]", { site: domain, id: item.key }),
						glyph: (item.country && icon("visit:country", item.country)) || undefined,
						meta: [
							stamp(item.started_at / 1000),
							span((item.ended_at - item.started_at) / 1000),
							`${item.pages} ${item.pages === 1 ? "page" : "pages"}`,
							`${item.clicks} ${item.clicks === 1 ? "click" : "clicks"}`,
							[item.browser, item.os].filter(Boolean).join(" on "),
							display("visit:country", item.country)
						].filter(Boolean)
					})) ?? null}
					remove={editable ? remove : undefined}
				/>
			</div>
			{#if more}
				<div class="flex justify-center pb-3">
					<Button variant="quiet" onclick={() => load(replays?.at(-1)?.started_at)}
						>Show more</Button
					>
				</div>
			{/if}
		{/if}
	</section>
</div>
