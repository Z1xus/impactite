<script lang="ts">
	import "./layout.css";
	import font from "#lib/fonts/DepartureMono-Regular.woff2?url";
	import { goto, onNavigate } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { ApiError } from "#lib/api.ts";
	import { refresh, session } from "#lib/session.svelte.ts";
	import { toast } from "#lib/toast.svelte.ts";
	import type { LayoutProps } from "./$types";

	let { children }: LayoutProps = $props();

	const open = ["/setup", "/login", "/invite", "/signup", "/share", "/demo"];

	$effect(() => {
		refresh();
	});

	// where this visitor has to go instead, so a page never loads for someone it isn't for
	const redirect = $derived.by((): "/setup" | "/login" | "/" | null => {
		const path = page.url.pathname;
		// shared dashboards stay readable before anyone has set the server up
		if (session.setup)
			return ["/setup", "/share", "/demo"].some((prefix) => path.startsWith(prefix))
				? null
				: "/setup";
		if (!session.me) return open.some((prefix) => path.startsWith(prefix)) ? null : "/login";
		if (path.startsWith("/admin") && !session.me.admin) return "/";
		return null;
	});

	$effect(() => {
		if (session.ready && redirect) goto(resolve(redirect));
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

<!-- a failed background request shows once instead of failing silently -->
<svelte:window
	onunhandledrejection={(event) => {
		if (!(event.reason instanceof ApiError)) return;
		// a session that ended elsewhere sends you to sign in
		if (event.reason.status === 401 && session.me) refresh();
		else toast(event.reason.message, true);
	}}
/>

<svelte:head>
	<title>impactite</title>
	<link rel="preload" href={font} as="font" type="font/woff2" crossorigin="anonymous" />
</svelte:head>

{#if session.ready && !redirect}
	{@render children()}
{/if}
