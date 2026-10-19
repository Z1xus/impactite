<script lang="ts">
	import { page } from "$app/state";
	import { session } from "#lib/session.svelte.ts";
	import Failed from "#lib/ui/Failed.svelte";
	import Missing from "#lib/ui/Missing.svelte";
	import Shell from "#lib/ui/Shell.svelte";
</script>

<svelte:head>
	<title>{page.status === 404 ? "Not found" : "Error"} · impactite</title>
</svelte:head>

{#if page.status === 404 && session.me}
	<Shell>
		<Missing text="This page doesn't exist." />
	</Shell>
{:else}
	<Failed
		text={page.status === 404 ? "This page doesn't exist." : "Something went wrong."}
		retry={page.url.pathname.startsWith("/share/") ? () => location.reload() : undefined}
	/>
{/if}
