<script lang="ts">
	import { icon } from "#lib/icons/resolve.ts";
	import { theme } from "#lib/theme.svelte.ts";
	import Glyph from "#lib/ui/Glyph.svelte";
	import Logo from "#lib/ui/Logo.svelte";
	import State from "#lib/ui/State.svelte";
	import type { Look, Monitor } from "./types.ts";

	let {
		look,
		title,
		monitors
	}: { look: Look; title: string; monitors: Pick<Monitor, "id" | "name" | "state" | "strip">[] } =
		$props();

	const home = $derived.by(() => {
		try {
			return look.home ? new URL(look.home).hostname : "";
		} catch {
			return "";
		}
	});
	const down = $derived(monitors.filter((item) => item.state === "down").length);
	const tone = (cell: Monitor["strip"][number]) =>
		!cell
			? "bg-faint/30"
			: cell[1] === "down"
				? "bg-bad"
				: cell[1] === "slow"
					? "bg-warn"
					: "bg-cyan";
</script>

<div
	data-theme={look.theme === "auto" ? theme.shown : look.theme}
	data-style={look.style}
	data-accent={look.accent}
	class="flex flex-col overflow-hidden rounded-px border border-edge bg-ink text-fg"
	aria-hidden="true"
>
	{#if home || look.brand}
		<div class="flex h-9 items-center gap-2 border-b border-line px-3 text-small">
			{#if home}
				<Glyph spec={icon("event:hostname", home)} fallback={home} />
				{home}
			{:else}
				<Logo size={14} />
				<span class="font-mono">impactite</span>
			{/if}
		</div>
	{/if}
	<div class="flex flex-col gap-2.5 p-3">
		{#if title}<span class="text-small">{title}</span>{/if}
		<span
			class="flex items-center gap-2 rounded-px border border-line bg-panel px-2.5 py-2 text-small"
		>
			<State state={down ? "down" : "up"} dot />
			{down ? `${down} ${down === 1 ? "service" : "services"} down` : "All systems up"}
		</span>
		<div class="flex flex-col gap-2.5 rounded-px border border-line bg-panel p-2.5">
			{#each monitors.slice(0, 4) as monitor (monitor.id)}
				<div class="flex flex-col gap-1.5">
					<span class="flex items-center justify-between gap-2 text-small">
						<span class="truncate">{monitor.name}</span>
						<State state={monitor.state} dot />
					</span>
					<span class="flex gap-px">
						{#each monitor.strip.slice(-30) as cell, i (i)}
							<span class="h-3 flex-1 {tone(cell)}"></span>
						{/each}
					</span>
				</div>
			{:else}
				<span class="meta">No services yet.</span>
			{/each}
		</div>
		{#if look.brand}
			<span class="self-center meta">Status by impactite</span>
		{/if}
	</div>
</div>
