<script lang="ts">
	import "./layout.css";
	import font from "#lib/fonts/DepartureMono-Regular.woff2?url";
	import { goto, onNavigate } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { ApiError } from "#lib/api.ts";
	import { keepScroll } from "#lib/scroll.ts";
	import { refresh, session } from "#lib/session.svelte.ts";
	import { toast } from "#lib/toast.svelte.ts";
	import Failed from "#lib/ui/Failed.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Progress from "#lib/ui/Progress.svelte";
	import type { LayoutProps } from "./$types";

	let { children }: LayoutProps = $props();

	const open = ["/setup", "/login", "/invite", "/signup", "/share", "/status", "/demo"];
	const auth = [...open, "/email"];

	keepScroll();

	$effect(() => {
		refresh();
	});

	onNavigate((navigation) => {
		if (
			!document.startViewTransition ||
			matchMedia("(prefers-reduced-motion: reduce)").matches ||
			navigation.from?.url.pathname === navigation.to?.url.pathname ||
			// a scrolled page would fade into one at the top
			(scrollY > 0 && navigation.type !== "popstate")
		)
			return;
		return new Promise((done) => {
			document.startViewTransition(async () => {
				done();
				await navigation.complete;
			});
		});
	});

	// where this visitor has to go instead, so a page never loads for someone it isn't for
	const redirect = $derived.by((): "/setup" | "/login" | "/" | null => {
		if (!session.ready) return null;
		const path = page.url.pathname;
		// shared dashboards stay readable before anyone has set the server up
		if (session.setup)
			return ["/setup", "/share", "/status", "/demo"].some((prefix) =>
				path.startsWith(prefix)
			)
				? null
				: "/setup";
		if (!session.me)
			return open.some((prefix) => path.startsWith(prefix)) ||
				(session.demo && path.startsWith("/settings"))
				? null
				: "/login";
		if (path.startsWith("/admin") && !session.me.admin) return "/";
		return null;
	});

	$effect(() => {
		if (redirect) goto(resolve(redirect));
	});

	// app pages draw their own skeleton before the session is known, auth pages wait behind the gate
	const shown = $derived(
		auth.some((prefix) => page.url.pathname.startsWith(prefix))
			? session.ready && !redirect
			: !redirect || !session.me
	);
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

<Progress />

{#if shown}
	<svelte:boundary>
		{@render children()}
		<!-- eslint-disable-next-line @typescript-eslint/no-unused-vars -- the snippet signature puts reset second -->
		{#snippet failed(error, reset)}
			<Failed text="Something went wrong." retry={reset} />
		{/snippet}
	</svelte:boundary>
{:else}
	<Gate />
{/if}
