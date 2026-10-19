<script lang="ts" generics="K extends string | number">
	import type { Snippet } from "svelte";
	import trash from "pixelarticons/svg/trash.svg?raw";
	import Avatar from "./Avatar.svelte";
	import Button from "./Button.svelte";
	import Empty from "./Empty.svelte";
	import Icon from "./Icon.svelte";
	import Skeleton from "./Skeleton.svelte";

	let {
		items,
		source,
		empty,
		remove,
		verb = "Delete",
		control
	}: {
		items:
			| {
					key: K;
					title: string;
					meta?: string[];
					removable?: boolean;
					verb?: string;
					// a person, null while their invite is open
					avatar?: "joined" | "invited";
			  }[]
			| null;
		source?: { failed: boolean; load: () => unknown };
		empty?: string;
		remove?: (key: K) => unknown;
		verb?: string;
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
	<ul class="-my-3 flex flex-col divide-y divide-line">
		{#if items === null}
			<li class="flex h-14 items-center"><Skeleton class="h-3 w-40" /></li>
		{:else}
			{#each items as item (item.key)}
				<li class="flex items-center gap-3 py-3">
					{#if item.avatar}
						<Avatar name={item.title} invited={item.avatar === "invited"} size={32} />
					{/if}
					<div class="flex min-w-0 flex-1 flex-col gap-1.5">
						<span class="truncate text-body text-fg">{item.title}</span>
						{#if item.meta?.length}
							<span
								class="flex flex-wrap gap-x-3 gap-y-1 label leading-4 tracking-normal normal-case"
							>
								{#each item.meta as part, i (i)}
									{#if i}<span class="h-2.5 w-px self-center bg-edge"></span>{/if}
									<span class="max-w-full truncate" title={part}>{part}</span>
								{/each}
							</span>
						{/if}
					</div>
					<div class="flex shrink-0 items-center gap-1">
						{@render control?.(item.key)}
						{#if remove && item.removable !== false}
							<Button
								variant="tool"
								class="-mr-3 hover:text-bad"
								aria-label="{item.verb ?? verb} {item.title}"
								title={item.verb ?? verb}
								disabled={removing === item.key}
								onclick={() => drop(item.key)}
							>
								<Icon svg={trash} size={12} />
							</Button>
						{/if}
					</div>
				</li>
			{/each}
		{/if}
	</ul>
{/if}
