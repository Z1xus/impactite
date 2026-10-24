<script lang="ts">
	import type { Site } from "#lib/api.ts";
	import Checks from "#lib/ui/Checks.svelte";
	import Note from "#lib/ui/Note.svelte";
	import Section from "#lib/ui/Section.svelte";
	import { saveSite } from "./site.ts";

	let { domain, site }: { domain: string; site: Site } = $props();

	const COLLECTED = [
		{ key: "referrer", label: "referrers" },
		{ key: "agent", label: "browsers and systems" },
		{ key: "device", label: "devices" },
		{ key: "location", label: "countries" },
		{ key: "region", label: "regions and cities" },
		{ key: "language", label: "languages" },
		{ key: "session", label: "sessions" },
		{ key: "vitals", label: "page speed" }
	];
	const RECORDED = [
		{ key: "heatmaps", label: "heatmaps" },
		{ key: "replays", label: "replays" }
	];
	const keys = (group: { key: string }[]) => group.map((item) => item.key);

	const collect = (group: { key: string }[], collected: string[]) =>
		saveSite(domain, {
			skip: [
				...site.skip.filter((key) => !keys(group).includes(key)),
				...keys(group).filter((key) => !collected.includes(key))
			]
		});
</script>

<Section id="collection" label="collection">
	{#if site.skip.includes("session")}
		<Note tone="warn">Without sessions, every pageview will count as a new visitor.</Note>
	{/if}
	<Checks
		bind:value={
			() => keys(COLLECTED).filter((key) => !site.skip.includes(key)),
			(collected) => collect(COLLECTED, collected)
		}
		options={COLLECTED}
	/>
	<Checks
		bind:value={
			() => keys(RECORDED).filter((key) => !site.skip.includes(key)),
			(recorded) => collect(RECORDED, recorded)
		}
		options={RECORDED}
	/>
	{#if keys(RECORDED).some((key) => !site.skip.includes(key))}
		<Note>
			Recordings hide all page text and anything visitors type. To show some text, add
			<code class="font-code text-fg">data-impactite-unmask</code> to it.
		</Note>
	{/if}
</Section>
