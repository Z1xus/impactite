<script lang="ts">
	import { clock, date } from "#lib/display.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Section from "#lib/ui/Section.svelte";
	import type { Admin } from "./types.ts";
	import Note from "#lib/ui/Note.svelte";

	let { info, checking, check }: { info: Admin; checking: boolean; check: () => void } = $props();
</script>

<Section id="version" label="version">
	{#snippet actions()}
		{#if info && info.update.enabled}
			<Button variant="text" busy={checking || info.update.checking} onclick={check}>
				{checking || info.update.checking ? "Checking…" : "Check now"}
			</Button>
		{/if}
	{/snippet}
	{#if info.update.latest}
		<div class="flex flex-col gap-4">
			<div class="flex items-baseline justify-between gap-4 text-[14px]">
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
				class="grid grid-cols-[auto_auto_1fr] items-baseline gap-x-4 gap-y-2 rounded-px border border-line bg-panel px-3 py-3 text-[14px] text-fg"
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
			<Note>
				Run <code class="font-mono text-[12px] text-cyan">impactite update</code> or pull the
				new Docker image, then restart.
			</Note>
		</div>
	{:else}
		<Note>
			{#if !info.update.enabled}
				impactite {info.version}. Update checks are disabled.
			{:else if checking || info.update.checking}
				impactite {info.version}. Checking for updates…
			{:else if info.update.failed}
				impactite {info.version}. Could not check for updates.
			{:else if info.update.checked_at !== null}
				impactite {info.version} is up to date.
			{:else}
				impactite {info.version}. Updates have not been checked yet.
			{/if}
		</Note>
	{/if}
	{#if info.update.latest && info.update.failed}
		<Note>Could not refresh update information.</Note>
	{/if}
	{#if info.update.checked_at !== null}
		<Note>
			Last successful check:
			<time datetime={new Date(info.update.checked_at * 1000).toISOString()}>
				{date(info.update.checked_at)} at {clock(
					new Date(info.update.checked_at * 1000).toTimeString().slice(0, 5)
				)}
			</time>
		</Note>
	{/if}
</Section>
