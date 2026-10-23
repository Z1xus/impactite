<script lang="ts">
	import { tick } from "#lib/uptime/clock.svelte.ts";
	import { span } from "#lib/uptime/format.ts";
	import type { Health } from "#lib/uptime/types.ts";

	let { state, since, dot = false }: { state: Health; since?: number; dot?: boolean } = $props();

	const looks: Record<Health, { dot: string; text: string }> = {
		up: { dot: "bg-cyan", text: "text-fg" },
		slow: { dot: "bg-warn", text: "text-warn" },
		down: { dot: "bg-bad", text: "text-bad" },
		paused: { dot: "bg-control", text: "text-dim" },
		maintenance: { dot: "bg-control", text: "text-dim" },
		pending: { dot: "bg-faint", text: "text-faint" }
	};
	const look = $derived(looks[state]);
</script>

{#if dot}
	<span role="img" aria-label={state} title={state} class="h-1.5 w-1.5 shrink-0 {look.dot}"
	></span>
{:else}
	<span class="flex items-center gap-2 whitespace-nowrap">
		<span class="h-1.5 w-1.5 shrink-0 {look.dot}"></span>
		<span class="label {look.text}">
			{state}{state === "down" && since ? ` ${span(tick.now - since)}` : ""}
		</span>
	</span>
{/if}
