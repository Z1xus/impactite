<script lang="ts">
	import down from "pixelarticons/svg/arrow-down.svg?raw";
	import up from "pixelarticons/svg/arrow-up.svg?raw";
	import type { Spec } from "#lib/icons/resolve.ts";
	import Glyph from "./Glyph.svelte";
	import Icon from "./Icon.svelte";

	type Column = { key: string; label: string; format: (value: number | null) => string };

	let {
		columns,
		rows,
		sort = $bindable(),
		pick
	}: {
		columns: Column[];
		rows:
			{ key: string; label: string; icon?: Spec | null; values: (number | null)[] }[] | null;
		sort: [string, "asc" | "desc"];
		pick?: (key: string) => void;
	} = $props();

	function order(key: string) {
		sort = [key, sort[0] === key && sort[1] === "desc" ? "asc" : "desc"];
	}
</script>

<div class="overflow-x-auto">
	<table class="w-full border-collapse text-left">
		<thead>
			<tr class="border-b border-line">
				<th class="w-full min-w-24 py-2 pr-4 font-normal"></th>
				{#each columns as column (column.key)}
					<th class="py-2 pl-4 text-right font-normal whitespace-nowrap">
						<button
							type="button"
							class="inline-flex cursor-pointer items-center gap-1 label transition-colors duration-100 hover:text-fg {sort[0] ===
							column.key
								? 'text-fg'
								: ''}"
							onclick={() => order(column.key)}
						>
							{column.label}
							{#if sort[0] === column.key}
								<Icon svg={sort[1] === "desc" ? down : up} size={10} />
							{/if}
						</button>
					</th>
				{/each}
			</tr>
		</thead>
		<tbody>
			{#if rows === null}
				{#each { length: 8 }, i (i)}
					<tr
						><td colspan={columns.length + 1} class="h-9"
							><span class="block h-3 w-1/2 rounded-px dither opacity-60"></span></td
						></tr
					>
				{/each}
			{:else}
				{#each rows as row (row.key)}
					<tr class="group border-b border-line/60 last:border-0">
						<td class="w-full max-w-0 min-w-24 py-2 pr-4">
							<button
								type="button"
								class="flex w-full min-w-0 items-center gap-2.5 text-left enabled:cursor-pointer"
								disabled={!pick}
								onclick={() => pick?.(row.key)}
							>
								{#if row.icon !== undefined}
									<Glyph spec={row.icon} fallback={row.label} />
								{/if}
								<span class="truncate text-[14px] text-fg group-hover:text-cyan"
									>{row.label}</span
								>
							</button>
						</td>
						{#each columns as column, i (column.key)}
							<td
								class="py-2 pl-4 text-right num text-[13px] whitespace-nowrap text-dim"
							>
								{column.format(row.values[i])}
							</td>
						{/each}
					</tr>
				{/each}
			{/if}
		</tbody>
	</table>
</div>
