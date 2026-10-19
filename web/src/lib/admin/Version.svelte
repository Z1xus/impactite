<script lang="ts">
	import { api } from "#lib/api.ts";
	import { clock, date } from "#lib/display.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Note from "#lib/ui/Note.svelte";
	import Section from "#lib/ui/Section.svelte";
	import { admin, load } from "./admin.svelte.ts";
	import type { Admin } from "./types.ts";

	let { info }: { info: Admin } = $props();

	let asking = $state(false);
	const checking = $derived(asking || info.update.checking);
	const checked = $derived(info.update.checked_at);

	$effect(() => {
		if (!info.update.checking) return;
		const timer = setTimeout(load, 1000);
		return () => clearTimeout(timer);
	});

	const check = () =>
		attempt(async () => {
			asking = true;
			try {
				admin.info = await api<Admin>("/api/app/admin/update-check", {});
			} finally {
				asking = false;
			}
		});
</script>

<Section id="version" label="version">
	{#snippet actions()}
		{#if info.update.enabled}
			<Button variant="quiet" busy={checking} onclick={check}>
				{checking ? "Checking…" : "Check now"}
			</Button>
		{/if}
	{/snippet}
	{#if info.update.latest}
		<div class="flex items-baseline justify-between gap-4 text-body">
			<span class="text-fg">
				impactite {info.update.latest.version} is available.
				<span class="text-dim">You're on {info.version}.</span>
			</span>
			<!-- eslint-disable svelte/no-navigation-without-resolve -- the release page on github -->
			<a href={info.update.latest.url} class="shrink-0 text-cyan hover:text-fg"
				>Release notes</a
			>
			<!-- eslint-enable svelte/no-navigation-without-resolve -->
		</div>
		<div
			class="grid grid-cols-[auto_auto_1fr] items-baseline gap-x-4 gap-y-2 rounded-px bg-raised px-3 py-3 text-body text-fg"
		>
			{#each info.update.latest.changes as change (change.version)}
				{#each change.lines as line, i (i)}
					{@const at = line.indexOf(": ")}
					<span class="label text-dim">{i === 0 ? change.version : ""}</span>
					<span class="label text-cyan">{at > 0 ? line.slice(0, at) : ""}</span>
					<span>{at > 0 ? line.slice(at + 2) : line}</span>
				{/each}
			{/each}
		</div>
		<Note>Run impactite update or pull the new image, then restart.</Note>
	{:else}
		<ul class="-my-3 flex flex-col divide-y divide-line text-body">
			<li class="flex items-center justify-between gap-4 py-3">
				<span class="label">installed</span>
				<span class="text-fg">impactite {info.version}</span>
			</li>
			<li class="flex items-center justify-between gap-4 py-3">
				<span class="label">check for updates</span>
				{#if !info.update.enabled}
					<span class="text-dim">Off</span>
				{:else if info.update.failed}
					<span class="text-warn">Could not check</span>
				{:else if checked !== null}
					<span class="text-dim">
						Up to date, checked {date(checked)} at {clock(
							new Date(checked * 1000).toTimeString().slice(0, 5)
						)}
					</span>
				{/if}
			</li>
		</ul>
	{/if}
</Section>
