<script lang="ts">
	import { tick, type Snippet } from "svelte";
	import { goto } from "$app/navigation";
	import { page } from "$app/state";
	import { resolve } from "$app/paths";
	import user from "pixelarticons/svg/user.svg?raw";
	import users from "pixelarticons/svg/users.svg?raw";
	import { api } from "#lib/api.ts";
	import { adopt, currentTeam, pickTeam, refresh, session } from "#lib/session.svelte.ts";
	import { toast } from "#lib/toast.svelte.ts";
	import Dialog from "./Dialog.svelte";
	import Field from "./Field.svelte";
	import Form from "./Form.svelte";
	import Logo from "./Logo.svelte";
	import Menu from "./Menu.svelte";
	import Skeleton from "./Skeleton.svelte";
	import Ask from "./Ask.svelte";
	import Toasts from "./Toasts.svelte";

	let { children, actions }: { children: Snippet; actions?: Snippet } = $props();

	let creating = $state(false);
	let name = $state("");

	$effect(() => {
		const version = session.update;
		if (!version || localStorage.getItem("update") === version) return;
		localStorage.setItem("update", version);
		toast(`impactite ${version} is available.`, false, {
			label: "Details",
			href: resolve("/admin/[[section]]", { section: "version" })
		});
	});

	const team = $derived(
		session.me?.teams.find((item) =>
			item.sites.some((site) => site.domain === page.params.site)
		) ?? currentTeam()
	);
	const workspace = $derived([
		...(session.me?.teams ?? []).map((item) => ({
			label: item.personal ? "Personal" : item.name,
			icon: item.personal ? user : users,
			current: item.id === team?.id,
			onclick: (): void => {
				pickTeam(item.id);
				goto(resolve("/"));
			}
		})),
		"divider" as const,
		...(team && team.id !== 0 && !team.personal
			? [{ label: "Team settings", href: resolve("/team/[[section]]", {}) }]
			: []),
		{
			label: "Add team",
			onclick: (): void => {
				creating = true;
			}
		}
	]);
	const account = $derived([
		{ label: "Account", href: resolve("/settings/[[section]]", {}) },
		...(session.me?.admin
			? [{ label: "Instance", href: resolve("/admin/[[section]]", {}) }]
			: []),
		"divider" as const,
		{ label: "Sign out", onclick: (): void => void signOut() }
	]);

	async function create() {
		const created = await api<{ id: number }>("/api/app/teams", { name });
		name = "";
		creating = false;
		await refresh();
		pickTeam(created.id);
		await goto(resolve("/"));
		await tick();
		document.querySelector<HTMLElement>("header [aria-haspopup=menu]")?.focus();
	}

	async function signOut() {
		await api("/api/app/logout", {});
		adopt(null);
		await goto(resolve("/login"));
	}
</script>

<Dialog title="add team" bind:open={creating}>
	<Form action="Add team" submit={create}>
		<Field label="name" placeholder="Acme" maxlength={80} bind:value={name} required />
	</Form>
</Dialog>

<div class="min-h-dvh">
	<header class="relative z-40 border-b border-line [view-transition-name:shell]">
		<div class="mx-auto flex h-14 max-w-page items-center gap-2 gutter sm:gap-3">
			<!-- eslint-disable svelte/no-navigation-without-resolve -- a visitor on a shared dashboard stays on it -->
			<a
				href={session.me ? resolve("/") : page.url.pathname}
				aria-label="impactite"
				class="-mr-1 -ml-2 flex h-control min-w-control items-center justify-center gap-2 px-2 font-mono text-body text-fg"
			>
				<Logo />
				<span class="hidden sm:inline">impactite</span>
			</a>
			<!-- eslint-enable svelte/no-navigation-without-resolve -->
			{#if team}
				<Menu
					data
					label={team.personal ? "Personal" : team.name}
					icon={team.personal ? user : users}
					items={workspace}
				/>
			{:else if !session.ready}
				<Skeleton class="h-control w-32" />
			{/if}
			<div class="ml-auto flex items-center gap-3">
				{@render actions?.()}
				{#if session.me}
					<Menu label={session.me.name} avatar data align="right" items={account} />
				{:else if !session.ready}
					<Skeleton class="h-control w-28" />
				{/if}
			</div>
		</div>
	</header>
	<main
		class="mx-auto max-w-page gutter py-6 pb-[max(2rem,env(safe-area-inset-bottom))] sm:py-8 sm:pb-[max(2rem,env(safe-area-inset-bottom))]"
	>
		{@render children()}
	</main>
</div>

<Toasts />
<Ask />
