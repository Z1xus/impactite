<script lang="ts">
	import { copier } from "#lib/toast.svelte.ts";
	import Button from "./Button.svelte";

	let { text }: { text: string } = $props();

	const clipboard = copier();
</script>

<Button variant="plain" onclick={() => clipboard.copy(text)}>
	{clipboard.copied ? "Copied" : "Copy"}
</Button>
