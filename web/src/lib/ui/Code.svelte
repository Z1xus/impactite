<script lang="ts">
	import check from "pixelarticons/svg/check.svg?raw";
	import copy from "pixelarticons/svg/copy.svg?raw";
	import { copier } from "#lib/toast.svelte.ts";
	import Icon from "./Icon.svelte";

	let { code }: { code: string } = $props();

	const clipboard = copier();
</script>

<div class="flex items-start gap-2 rounded-px bg-raised py-2.5 pr-2 pl-3">
	<code
		class="min-w-0 flex-1 overflow-x-auto py-0.5 font-code text-small leading-5 whitespace-pre-wrap text-cyan"
		>{#each code.split(/(?<=[/?&])/) as part, i (i)}{part}<wbr />{/each}</code
	>
	<button
		type="button"
		class="-my-1.5 grid size-control shrink-0 place-items-center text-dim hover:text-fg"
		aria-label={clipboard.copied ? "copied" : "copy"}
		title={clipboard.copied ? "copied" : "copy"}
		onclick={() => clipboard.copy(code)}
	>
		<Icon svg={clipboard.copied ? check : copy} size={12} />
	</button>
</div>
