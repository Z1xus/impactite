<script lang="ts">
	import check from "pixelarticons/svg/check.svg?raw";
	import copy from "pixelarticons/svg/copy.svg?raw";
	import { copy as write } from "#lib/toast.svelte.ts";
	import Icon from "./Icon.svelte";

	let { code }: { code: string } = $props();

	let copied = $state(false);

	async function copyCode() {
		if (!(await write(code))) return;
		copied = true;
		setTimeout(() => (copied = false), 1200);
	}
</script>

<div class="flex items-start gap-2 rounded-px border border-line bg-panel py-2.5 pr-2 pl-3">
	<code
		class="min-w-0 flex-1 py-0.5 font-mono text-small leading-5 break-all whitespace-pre-wrap text-cyan"
		>{code}</code
	>
	<button
		type="button"
		class="grid h-6 w-6 shrink-0 cursor-pointer place-items-center text-dim transition-colors duration-100 hover:text-fg"
		aria-label="copy"
		title="copy"
		onclick={copyCode}
	>
		<Icon svg={copied ? check : copy} size={12} />
	</button>
</div>
