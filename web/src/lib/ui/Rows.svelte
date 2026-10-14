<script lang="ts">
	import external from "pixelarticons/svg/external-link.svg?raw";
	import { flip } from "svelte/animate";
	import type { Spec } from "#lib/icons/resolve.ts";
	import Glyph from "./Glyph.svelte";
	import Icon from "./Icon.svelte";
	import Num from "./Num.svelte";

	let {
		rows,
		loading = false,
		limit = 8,
		format,
		pick
	}: {
		rows:
			| {
					key: string;
					label: string;
					value: number;
					icon?: Spec | null;
					fallback?: string;
					href?: string;
					extra?: string;
			  }[]
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
			<li
				animate:flip={{ duration: 200 }}
				class="group relative flex h-8 items-center gap-2.5 px-2"
			>
				<button
					type="button"
					class="absolute inset-0 rounded-px enabled:cursor-pointer"
					aria-label={row.label}
					onclick={() => pick?.(row.key)}
					disabled={!pick}
				></button>
				{#if row.icon !== undefined}
					<Glyph spec={row.icon} fallback={row.fallback} />
				{/if}
				<span
					class="pointer-events-none relative flex h-full min-w-0 flex-1 items-center justify-between gap-4"
				>
					<span
						class="dither absolute inset-y-1 left-0 rounded-px transition-[width] duration-300 ease-out"
						style:width="{(row.value / max) * 100}%"
					></span>
					<span class="relative flex min-w-0 items-center gap-1.5 pl-2">
						<span class="truncate text-[14px] text-fg">{row.label}</span>
						{#if row.href}
							<!-- eslint-disable svelte/no-navigation-without-resolve -- external page -->
							<a
								href={row.href}
								target="_blank"
								rel="noopener noreferrer"
								aria-label="open {row.label}"
								class="pointer-events-auto grid h-5 w-5 shrink-0 cursor-pointer place-items-center text-dim opacity-0 transition-opacity duration-100 group-hover:opacity-100 hover:text-fg focus-visible:opacity-100"
							>
								<Icon svg={external} size={12} />
							</a>
							<!-- eslint-enable svelte/no-navigation-without-resolve -->
						{/if}
					</span>
					<span class="relative flex items-center gap-4 pr-2">
						{#if row.extra}
							<span class="text-label text-dim">{row.extra}</span>
						{/if}
						<Num
							value={row.value}
							{format}
							class="text-label text-dim group-hover:text-fg"
						/>
					</span>
				</span>
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
