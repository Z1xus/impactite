<script lang="ts">
	import check from "pixelarticons/svg/check.svg?raw";
	import copy from "pixelarticons/svg/copy.svg?raw";
	import { copier } from "#lib/toast.svelte.ts";
	import Button from "./Button.svelte";
	import Icon from "./Icon.svelte";

	let { text }: { text: string } = $props();

	const clipboard = copier();
</script>

<Button
	variant="tool"
	aria-label={clipboard.copied ? "Copied" : "Copy link"}
	title={clipboard.copied ? "Copied" : "Copy"}
	onclick={() => clipboard.copy(text)}
>
	<Icon svg={clipboard.copied ? check : copy} size={12} />
</Button>
