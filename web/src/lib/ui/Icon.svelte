<script lang="ts">
	import { plain } from "#lib/icons/plain.ts";
	import { theme } from "#lib/theme.svelte.ts";

	let {
		svg,
		size = 16,
		class: className = ""
	}: { svg: string; size?: number; class?: string } = $props();

	const twin = $derived(theme.style === "plain" ? plain.get(svg) : undefined);
	const paths = $derived([...svg.matchAll(/ d="([^"]+)"/g)].map((match) => match[1]));
</script>

{#if twin}
	<svg
		class="shrink-0 {className}"
		width={size}
		height={size}
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<!-- eslint-disable-next-line svelte/no-at-html-tags -- bundled lucide files -->
		{@html twin.slice(twin.indexOf(">", twin.indexOf("<svg")) + 1, twin.lastIndexOf("</svg>"))}
	</svg>
{:else}
	<svg
		class="shrink-0 {className}"
		width={size}
		height={size}
		viewBox="0 0 24 24"
		fill="currentColor"
		aria-hidden="true"
	>
		{#each paths as d, i (i)}
			<path {d} />
		{/each}
	</svg>
{/if}
