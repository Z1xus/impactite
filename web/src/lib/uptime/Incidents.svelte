<script lang="ts">
	import { api } from "#lib/api.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import chevron from "pixelarticons/svg/chevron-down.svg?raw";
	import trash from "pixelarticons/svg/trash.svg?raw";
	import Button from "#lib/ui/Button.svelte";
	import Code from "#lib/ui/Code.svelte";
	import Empty from "#lib/ui/Empty.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import { tick } from "./clock.svelte.ts";
	import { missed, span, stamp } from "./format.ts";
	import type { Incident } from "./types.ts";

	let {
		incidents,
		editable,
		reload
	}: { incidents: Incident[]; editable: boolean; reload: () => unknown } = $props();

	let opened = $state<number | null>(null);
	let text = $state("");
	let visible = $state<"public" | "private">("public");

	async function add(incident: Incident) {
		await api(`/api/app/uptime/incidents/${incident.id}/notes`, {
			text,
			public: visible === "public"
		});
		text = "";
		await reload();
	}

	const drop = (incident: number, note: number) =>
		attempt(async () => {
			await api(`/api/app/uptime/incidents/${incident}/notes/${note}`, undefined, "DELETE");
			await reload();
		});
</script>

{#if !incidents.length}
	<Empty compact text="No incidents." />
{:else}
	<ul class="-my-1 flex flex-col divide-y divide-line">
		{#each incidents as incident (incident.id)}
			{@const ongoing = incident.ended_at === null}
			{@const shown = opened === incident.id}
			<li>
				<button
					type="button"
					class="flex min-h-14 w-full items-center gap-3 py-3 text-left"
					aria-expanded={shown}
					onclick={() => (opened = shown ? null : incident.id)}
				>
					<span class="h-1.5 w-1.5 shrink-0 {ongoing ? 'bg-bad' : 'bg-faint'}"></span>
					<span class="flex min-w-0 flex-1 flex-col gap-1.5">
						<span class="truncate text-body text-fg">{incident.cause}</span>
						<span class="flex flex-wrap gap-x-3 meta">
							<span>{stamp(incident.started_at)}</span>
							{#if incident.visits_missed}
								<span>{missed(incident.visits_missed)}</span>
							{/if}
						</span>
					</span>
					<span
						class="num text-small whitespace-nowrap {ongoing ? 'text-bad' : 'text-fg'}"
					>
						{ongoing ? "ongoing" : span(incident.duration)}
					</span>
					<Icon svg={chevron} size={12} class="text-dim {shown ? 'rotate-180' : ''}" />
				</button>
				{#if shown}
					<div class="flex flex-col gap-4 pt-1 pb-4 pl-4.5">
						{#if incident.detail}
							<Code code={incident.detail} />
						{/if}
						{#if ongoing}
							<span class="meta">down for {span(tick.now - incident.started_at)}</span
							>
						{/if}
						{#if incident.notes.length}
							<ul class="flex flex-col gap-3">
								{#each incident.notes as note (note.id)}
									<li class="flex items-start gap-3">
										<div class="flex min-w-0 flex-1 flex-col gap-1.5">
											<span class="text-body break-words text-fg"
												>{note.text}</span
											>
											<span class="meta">
												{stamp(note.at)}{note.public ? "" : ", team only"}
											</span>
										</div>
										{#if editable}
											<Button
												variant="tool"
												class="-mt-1 -mr-2 hover:text-bad"
												aria-label="Delete note"
												title="Delete"
												onclick={() => drop(incident.id, note.id)}
											>
												<Icon svg={trash} size={12} />
											</Button>
										{/if}
									</li>
								{/each}
							</ul>
						{/if}
						{#if editable}
							<Form
								inline
								action="Add note"
								done="Added."
								submit={() => add(incident)}
							>
								<Field label="note" maxlength={500} bind:value={text} required />
								<Toggle
									bind:value={visible}
									options={[
										{ key: "public", label: "On status page" },
										{ key: "private", label: "Team only" }
									]}
								/>
							</Form>
						{/if}
					</div>
				{/if}
			</li>
		{/each}
	</ul>
{/if}
