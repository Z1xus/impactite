<script lang="ts" generics="K extends string | number">
	import type { Snippet } from "svelte";
	import close from "pixelarticons/svg/close.svg?raw";
	import Avatar from "./Avatar.svelte";
	import Empty from "./Empty.svelte";
	import Icon from "./Icon.svelte";
	import Skeleton from "./Skeleton.svelte";

	let {
		items,
		source,
		empty,
		remove,
		control
	}: {
		items:
			| {
					key: K;
					title: string;
					meta?: string[];
					removable?: boolean;
					// a person, null while their invite is open
					avatar?: number | null;
			  }[]
			| null;
		source?: { failed: boolean; load: () => unknown };
		empty?: string;
		remove?: (key: K) => unknown;
		control?: Snippet<[K]>;
	} = $props();

	let removing = $state<K | null>(null);

	async function drop(key: K) {
		removing = key;
		try {
			await remove?.(key);
		} finally {
			removing = null;
		}
	}
</script>

{#if items === null && source?.failed}
	<Empty compact retry={source.load} />
{:else if items?.length === 0 && empty}
	<Empty compact text={empty} />
{:else if items === null || items.length}
	<ul class="flex flex-col rounded-px border border-line bg-panel">
		{#if items === null}
			<li class="flex h-14 items-center px-4"><Skeleton class="h-3 w-40" /></li>
		{:else}
			{#each items as item (item.key)}
				<li
					class="flex flex-wrap items-center justify-between gap-x-4 gap-y-3 border-b border-line px-4 py-3 last:border-0"
				>
					{#if item.avatar !== undefined}
						{#if item.avatar === null}
							<span
								class="size-[32px] shrink-0 rounded-full border border-dashed border-edge"
							></span>
						{:else}
							<Avatar seed={item.avatar} size={32} />
						{/if}
					{/if}
					<div class="flex min-w-40 flex-1 flex-col gap-1.5">
						<span class="truncate text-body text-fg">{item.title}</span>
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
							class="-mr-2.5 grid h-control w-control shrink-0 place-items-center text-dim hover:text-bad"
							aria-label="remove {item.title}"
							disabled={removing === item.key}
							onclick={() => drop(item.key)}
						>
							<Icon svg={close} size={14} />
						</button>
					{/if}
				</li>
			{/each}
		{/if}
	</ul>
{/if}
