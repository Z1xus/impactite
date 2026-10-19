<script lang="ts">
	import { resolve } from "$app/paths";
	import Button from "./Button.svelte";
	import Empty from "./Empty.svelte";

	let { text = "This site doesn't exist, or you don't have access to it." }: { text?: string } =
		$props();
</script>

<svelte:head>
	<title>Not found · impactite</title>
</svelte:head>

<Empty {text}>
	<Button variant="quiet" href={resolve("/")}>All sites</Button>
</Empty>
