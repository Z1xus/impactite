<script lang="ts">
	import { flip } from "svelte/animate";
	import AddPanel from "#lib/ui/AddPanel.svelte";
	import Card from "./Card.svelte";
	import type { createLayout } from "./layout.svelte.ts";
	import { PANELS } from "./panels.ts";
	import type { Context, PanelSpec } from "./types.ts";

	let { ctx, layout }: { ctx: Context; layout: ReturnType<typeof createLayout> } = $props();

	let dragging = $state<string | null>(null);
	let dropTarget = $state<string | null>(null);

	const panels = $derived<PanelSpec[]>([
		...PANELS,
		{
			id: "goals",
			tabs: [
				...(ctx.goals.length ? [{ key: "event:goal", label: "goals" }] : []),
				...(ctx.propKeys.length ? [{ key: "props", label: "props" }] : []),
				...(ctx.funnels.length ? [{ key: "funnels", label: "funnels" }] : [])
			]
		}
	]);
	const arranged = $derived.by(() => {
		const all = panels.filter((panel) => panel.tabs.length);
		const rank = (id: string) => {
			const at = layout.order.indexOf(id);
			return at < 0 ? 1000 + all.findIndex((panel) => panel.id === id) : at;
		};
		return [...all].sort((a, b) => rank(a.id) - rank(b.id));
	});
	const visible = $derived(
		layout.arranging ? arranged : arranged.filter((panel) => !layout.isHidden(panel.id))
	);

	function over(event: DragEvent) {
		if (!dragging) return;
		event.preventDefault();
		if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
		const grid = event.currentTarget as HTMLElement;
		let closest: string | null = null;
		let distance = Infinity;
		for (const panel of grid.querySelectorAll<HTMLElement>(":scope > [data-panel]")) {
			const box = panel.getBoundingClientRect();
			const dx = Math.max(box.left - event.clientX, 0, event.clientX - box.right);
			const dy = Math.max(box.top - event.clientY, 0, event.clientY - box.bottom);
			const next = dx * dx + dy * dy;
			if (next < distance) {
				distance = next;
				closest = panel.dataset.panel ?? null;
			}
		}
		dropTarget = closest === dragging ? null : closest;
	}

	function drop(event: DragEvent) {
		if (!dragging) return;
		over(event);
		if (dropTarget) {
			const from = arranged.findIndex((panel) => panel.id === dragging);
			const to = arranged.findIndex((panel) => panel.id === dropTarget);
			const ids = arranged.map((panel) => panel.id).filter((id) => id !== dragging);
			ids.splice(ids.indexOf(dropTarget) + (from < to ? 1 : 0), 0, dragging);
			layout.setOrder(ids);
		}
		end();
	}

	function start(event: DragEvent, id: string) {
		dragging = id;
		dropTarget = null;
		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = "move";
			event.dataTransfer.setData("text/plain", id);
		}
	}

	function end() {
		dragging = null;
		dropTarget = null;
	}
</script>

<div
	class="grid grid-cols-1 gap-6 md:grid-cols-2"
	role="list"
	ondragover={over}
	ondrop={drop}
	ondragleave={(event) => {
		if (!(event.currentTarget as HTMLElement).contains(event.relatedTarget as Node | null))
			dropTarget = null;
	}}
>
	{#each visible as panel (panel.id)}
		{@const hidden = layout.isHidden(panel.id)}
		<div
			role="listitem"
			data-panel={panel.id}
			animate:flip={{ duration: 200 }}
			class="min-w-0 transition-opacity duration-150 {panel.id === 'goals' && !hidden
				? 'md:col-span-2'
				: ''} {dragging === panel.id ? 'opacity-40' : ''} {layout.arranging
				? 'select-none'
				: ''} {dropTarget === panel.id ? 'outline -outline-offset-1 outline-cyan' : ''}"
			draggable={layout.arranging && !hidden}
			ondragstart={(event) => start(event, panel.id)}
			ondragend={end}
		>
			{#if hidden}
				<AddPanel label={panel.tabs[0].label} onclick={() => layout.show(panel.id)} />
			{:else}
				<Card
					{panel}
					{ctx}
					arranging={layout.arranging}
					hide={() => layout.hide(panel.id)}
				/>
			{/if}
		</div>
	{/each}
</div>
