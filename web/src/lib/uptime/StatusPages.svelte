<script lang="ts">
	import pencil from "pixelarticons/svg/pencil.svg?raw";
	import { get } from "#lib/api.ts";
	import { collection } from "#lib/collection.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Copy from "#lib/ui/Copy.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import List from "#lib/ui/List.svelte";
	import Section from "#lib/ui/Section.svelte";
	import StatusPageDialog from "./StatusPageDialog.svelte";
	import type { Monitor, Page } from "./types.ts";

	let { team }: { team: number } = $props();

	const pages = collection<Page>(
		() => "/api/app/uptime/pages",
		"pages",
		() => `?team=${team}`
	);
	let monitors = $state.raw<Monitor[]>([]);
	let states = $state.raw<Record<number, Monitor["state"]>>({});
	let editing = $state<Page | undefined>();
	let open = $state(false);

	const url = (slug: string) => `${location.origin}/status/${slug}`;

	$effect(() => {
		pages.load();
		get<{ monitors: Monitor[] }>(`/api/app/uptime?team=${team}`).then((reply) => {
			monitors = reply.monitors;
			states = Object.fromEntries(reply.monitors.map((item) => [item.id, item.state]));
		});
	});

	function show(page?: Page) {
		editing = page;
		open = true;
	}

	function status(page: Page) {
		const down = page.monitors.filter((id) => states[id] === "down").length;
		if (down) return { text: `${down} down`, tone: "bad" as const };
		return page.monitors.length
			? { text: "All up" }
			: { text: "No monitors", tone: "warn" as const };
	}

	const remove = (id: string | number) =>
		attempt(() => pages.remove(id), {
			title: "Delete this status page?",
			body: "Its link and badges will stop working.",
			action: "Delete page"
		});
</script>

<StatusPageDialog
	bind:open
	page={editing}
	{monitors}
	save={(body) => (editing ? pages.change(editing.id, body) : pages.add({ ...body, team }))}
/>

<Section id="status-pages" label="status pages">
	{#snippet actions()}
		<Button variant="quiet" onclick={() => show()}>Add page</Button>
	{/snippet}
	<List
		items={pages.items?.map((page) => ({
			key: page.id,
			title: page.name || page.slug,
			status: status(page),
			meta: [
				url(page.slug),
				`${page.monitors.length} ${page.monitors.length === 1 ? "monitor" : "monitors"}`,
				...(page.locked ? ["password"] : [])
			]
		})) ?? null}
		source={pages}
		empty="No status pages yet."
		{remove}
	>
		{#snippet control(key)}
			{@const page = pages.items?.find((item) => item.id === key)}
			{#if page}
				<Copy text={url(page.slug)} />
				<Button
					variant="tool"
					aria-label="Edit {page.name}"
					title="Edit"
					onclick={() => show(page)}
				>
					<Icon svg={pencil} size={12} />
				</Button>
			{/if}
		{/snippet}
	</List>
</Section>
