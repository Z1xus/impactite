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
		{ key: "session", label: "sessions" }
	];
</script>

<Section id="collection" label="collection">
	{#if site.skip.includes("session")}
		<Note tone="warn">Without sessions, every pageview counts as a new visitor.</Note>
	{/if}
	<Checks
		bind:value={
			() => COLLECTED.map((item) => item.key).filter((key) => !site.skip.includes(key)),
			(collected) =>
				saveSite(domain, {
					skip: COLLECTED.map((item) => item.key).filter(
						(key) => !collected.includes(key)
					)
				})
		}
		options={COLLECTED}
	/>
</Section>
