<script lang="ts">
	import { flip } from "svelte/animate";
	import type { Spec } from "#lib/icons/lens.ts";
	import Glyph from "./Glyph.svelte";
	import Num from "./Num.svelte";

	let {
		rows,
		loading = false,
		limit = 8,
		format,
		pick
	}: {
		rows:
			| { key: string; label: string; value: number; icon?: Spec | null; fallback?: string }[]
			| null;
		loading?: boolean;
		limit?: number;
		format?: (value: number) => string;
		pick?: (key: string) => void;
	} = $props();

	const max = $derived(Math.max(1, ...(rows ?? []).map((row) => row.value)));
</script>

<ul class="flex flex-col transition-opacity duration-150 {loading ? 'opacity-60' : ''}">
	{#if rows === null}
		{#each { length: limit }, i (i)}
			<li class="flex h-8 items-center px-2">
				<span class="dither h-3 rounded-px" style:width="{70 - i * 7}%"></span>
			</li>
		{/each}
	{:else if rows.length === 0}
		<li class="flex h-8 items-center px-2 label">none</li>
	{:else}
		{#each rows.slice(0, limit) as row (row.key)}
			<li animate:flip={{ duration: 200 }}>
				<button
					type="button"
					class="group relative flex h-8 w-full items-center justify-between gap-4 rounded-px px-2 text-left enabled:cursor-pointer"
					onclick={() => pick?.(row.key)}
					disabled={!pick}
				>
					<span
						class="dither absolute inset-y-1 left-0 rounded-px transition-[width] duration-300 ease-out group-hover:opacity-100"
						style:width="{(row.value / max) * 100}%"
					></span>
					<span class="relative flex min-w-0 items-center gap-2.5">
						{#if row.icon !== undefined}
							<Glyph spec={row.icon} fallback={row.fallback} />
						{/if}
						<span class="truncate text-[14px] text-fg">{row.label}</span>
					</span>
					<Num
						value={row.value}
						{format}
						class="relative text-label text-dim group-hover:text-fg"
					/>
				</button>
			</li>
		{/each}
	{/if}
</ul>

<style>
	.dither {
		opacity: 0.85;
		background-color: var(--color-cyan-night);
		background-image:
			linear-gradient(45deg, var(--color-panel) 25%, transparent 25%),
			linear-gradient(-45deg, var(--color-panel) 25%, transparent 25%);
		background-size: 4px 4px;
	}
</style>
