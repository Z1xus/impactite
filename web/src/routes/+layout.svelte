<script lang="ts">
	import "./layout.css";
	import { goto, onNavigate } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { refresh, session } from "#lib/session.svelte.ts";
	import type { LayoutProps } from "./$types";

	let { children }: LayoutProps = $props();

	const open = ["/setup", "/login", "/invite"];

	$effect(() => {
		refresh();
	});

	$effect(() => {
		if (!session.ready) return;
		const path = page.url.pathname;
		if (session.setup && !path.startsWith("/setup")) goto(resolve("/setup"));
		else if (!session.setup && !session.me && !open.some((prefix) => path.startsWith(prefix)))
			goto(resolve("/login"));
	});

	onNavigate((navigation) => {
		if (!document.startViewTransition) return;
		return new Promise((resolve) => {
			document.startViewTransition(async () => {
				resolve();
				await navigation.complete;
			});
		});
	});
</script>

<svelte:head>
	<title>impactite</title>
</svelte:head>

{#if session.ready}
	{@render children()}
{/if}
