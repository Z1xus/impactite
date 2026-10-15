<script lang="ts">
	import type { Snippet } from "svelte";
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import user from "pixelarticons/svg/user.svg?raw";
	import users from "pixelarticons/svg/users.svg?raw";
	import { api } from "#lib/api.ts";
	import { adopt, currentTeam, pickTeam, refresh, session } from "#lib/session.svelte.ts";
	import Dialog from "./Dialog.svelte";
	import Field from "./Field.svelte";
	import Form from "./Form.svelte";
	import Menu from "./Menu.svelte";
	import Toasts from "./Toasts.svelte";

	let { children, actions }: { children: Snippet; actions?: Snippet } = $props();

	let creating = $state(false);
	let name = $state("");

	const team = $derived(currentTeam());
	const workspace = $derived([
		...(session.me?.teams ?? []).map((item) => ({
			label: item.personal ? "personal" : item.name,
			icon: item.personal ? user : users,
			current: item.id === session.team,
			onclick: (): void => {
				pickTeam(item.id);
				goto(resolve("/"));
			}
		})),
		"divider" as const,
		...(team && team.id !== 0 && !team.personal
			? [{ label: "team settings", href: resolve("/team") }]
			: []),
		{
			label: "new team",
			onclick: (): void => {
				creating = true;
			}
		}
	]);
	const account = $derived([
		{ label: "account", href: resolve("/settings") },
		...(session.me?.admin ? [{ label: "instance", href: resolve("/admin") }] : []),
		"divider" as const,
		{ label: "sign out", onclick: (): void => void signOut() }
	]);

	async function create() {
		const created = await api<{ id: number }>("/api/app/teams", { name });
		name = "";
		creating = false;
		await refresh();
		pickTeam(created.id);
		await goto(resolve("/"));
	}

	async function signOut() {
		await api("/api/app/logout", {});
		adopt(null);
		await goto(resolve("/login"));
	}
</script>

<Dialog title="new team" bind:open={creating}>
	<Form action="create team" submit={create}>
		<Field label="name" placeholder="Acme" bind:value={name} required />
	</Form>
</Dialog>

<div class="min-h-dvh">
	<header class="border-b border-line">
		<div class="mx-auto flex h-14 max-w-6xl items-center gap-3 px-6">
			<a
				href={resolve("/")}
				class="mr-1 flex items-center gap-2 font-mono text-[15px] text-fg"
			>
				<img src="/favicon.svg" alt="impactite" width="22" height="22" class="pixelated" />
				<span class="hidden sm:inline">impactite</span>
			</a>
			{#if team}
				<Menu
					label={team.personal ? "personal" : team.name}
					icon={team.personal ? user : users}
					items={workspace}
				/>
			{/if}
			<div class="ml-auto flex items-center gap-3">
				{@render actions?.()}
				{#if session.me}
					<Menu label={session.me.name} data align="right" items={account} />
				{/if}
			</div>
		</div>
	</header>
	<main class="mx-auto max-w-6xl px-6 py-8">
		{@render children()}
	</main>
</div>

<Toasts />
