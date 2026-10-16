<script lang="ts">
	import type { Snippet } from "svelte";
	import close from "pixelarticons/svg/close.svg?raw";
	import Avatar from "./Avatar.svelte";
	import Icon from "./Icon.svelte";

	type Key = string | number;

	let {
		items,
		remove,
		control
	}: {
		items:
			| {
					key: Key;
					title: string;
					meta?: string[];
					removable?: boolean;
					// a person, null while their invite is open
					avatar?: number | null;
			  }[]
			| null;
		remove?: (key: Key) => void;
		control?: Snippet<[Key]>;
	} = $props();
</script>

{#if items === null || items.length}
	<ul class="flex flex-col rounded-px border border-line bg-panel">
		{#if items === null}
			<li class="h-14"></li>
		{:else}
			{#each items as item (item.key)}
				<li
					class="flex flex-wrap items-center justify-between gap-x-4 gap-y-3 border-b border-line px-4 py-3 last:border-0"
				>
					{#if item.avatar !== undefined}
						{#if item.avatar === null}
							<span
								class="size-8 shrink-0 rounded-full border border-dashed border-edge"
							></span>
						{:else}
							<Avatar seed={item.avatar} size={32} />
						{/if}
					{/if}
					<div class="flex min-w-40 flex-1 flex-col gap-1.5">
						<span class="truncate text-[14px] text-fg">{item.title}</span>
						{#if item.meta?.length}
							<span
								class="flex flex-wrap gap-x-3 gap-y-1 label leading-4 tracking-normal normal-case"
							>
								{#each item.meta as part, i (i)}
									{#if i}<span class="h-2.5 w-px self-center bg-edge"></span>{/if}
									<span class="min-w-0 [overflow-wrap:anywhere]">{part}</span>
								{/each}
							</span>
						{/if}
					</div>
					{@render control?.(item.key)}
					{#if remove && item.removable !== false}
						<button
							type="button"
							class="-mr-2.5 grid h-9 w-9 shrink-0 cursor-pointer place-items-center text-dim transition-colors duration-100 hover:text-bad"
							aria-label="remove {item.title}"
							onclick={() => remove(item.key)}
						>
							<Icon svg={close} size={14} />
						</button>
					{/if}
				</li>
			{/each}
		{/if}
	</ul>
{/if}
