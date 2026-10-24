<script lang="ts">
	import external from "pixelarticons/svg/external-link.svg?raw";
	import type { Spec } from "#lib/icons/resolve.ts";
	import Glyph from "./Glyph.svelte";
	import Icon from "./Icon.svelte";
	import Num from "./Num.svelte";

	let {
		rows,
		loading = false,
		limit = 8,
		format,
		columns,
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
					note?: string;
					tone?: "warn" | "bad";
			  }[]
			| null;
		loading?: boolean;
		limit?: number;
		format?: (value: number) => string;
		columns?: string[];
		pick?: (key: string) => void;
	} = $props();

	const max = $derived(Math.max(1, ...(rows ?? []).map((row) => row.value)));
</script>

<ul class="flex flex-col transition-opacity duration-150 {loading ? 'opacity-60 delay-300' : ''}">
	{#if columns && rows?.length}
		<li class="flex h-6 items-start justify-end gap-4 px-4 label text-faint">
			{#each columns as column, i (column)}
				<span class={i === columns.length - 1 ? "min-w-12 text-right" : "w-14 text-right"}
					>{column}</span
				>
			{/each}
		</li>
	{/if}
	{#if rows === null}
		{#each { length: limit }, i (i)}
			<li class="flex h-8 items-center px-2">
				<span
					data-skeleton
					class="h-3 rounded-px dither opacity-85"
					style:width="{70 - i * 7}%"
				></span>
			</li>
		{/each}
	{:else if rows.length === 0}
		<li data-empty class="flex h-8 items-center px-2 label">none</li>
	{:else}
		{#each rows.slice(0, limit) as row, i (i)}
			<li class="group relative flex h-8 items-center gap-2.5 px-2 pointer-coarse:h-control">
				<button
					type="button"
					class="absolute inset-0 rounded-px enabled:hover:bg-raised"
					aria-label={row.label}
					onclick={() => pick?.(row.key)}
					inert={!pick}
				></button>
				{#if row.icon || row.fallback}
					<Glyph spec={row.icon ?? null} fallback={row.fallback} />
				{/if}
				<span
					class="pointer-events-none relative flex h-full min-w-0 flex-1 items-center justify-between gap-4"
				>
					<span
						class="relative isolate flex h-full min-w-0 flex-1 items-center gap-1.5 pl-2"
					>
						<span
							class="bar absolute inset-y-1 left-0 -z-10 rounded-px transition-[width] duration-300 ease-out motion-reduce:transition-none"
							style:width="{(row.value / max) * 100}%"
						></span>
						<span class="truncate text-body text-fg">{row.label}</span>
						{#if row.note}
							<span class="min-w-0 shrink-[2] truncate text-small text-dim"
								>{row.note}</span
							>
						{/if}
						{#if row.href}
							<!-- eslint-disable svelte/no-navigation-without-resolve -- external page -->
							<a
								href={row.href}
								target="_blank"
								rel="noopener noreferrer"
								aria-label="open {row.label}"
								class="pointer-events-auto -mx-1.5 -my-1.5 grid size-control shrink-0 place-items-center text-dim opacity-0 group-hover:opacity-100 hover:text-fg focus-visible:opacity-100 pointer-coarse:opacity-100"
							>
								<Icon svg={external} size={12} />
							</a>
							<!-- eslint-enable svelte/no-navigation-without-resolve -->
						{/if}
					</span>
					<span class="relative flex items-center gap-4 pr-2">
						{#if row.extra}
							<span class="w-14 text-right font-mono text-label text-dim"
								>{row.extra}</span
							>
						{/if}
						<Num
							value={row.value}
							{format}
							class="min-w-12 text-right text-label {row.tone === 'bad'
								? 'text-bad'
								: row.tone === 'warn'
									? 'text-warn'
									: 'text-dim group-hover:text-fg'}"
						/>
					</span>
				</span>
			</li>
		{/each}
	{/if}
</ul>
