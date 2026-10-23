<script lang="ts">
	import type { Snippet } from "svelte";
	import { page } from "$app/state";
	import { resolve } from "$app/paths";
	import { currentTeam } from "#lib/session.svelte.ts";
	import { writes } from "#lib/uptime/types.ts";
	import Nav from "#lib/ui/Nav.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Skeleton from "#lib/ui/Skeleton.svelte";
	import Title from "#lib/ui/Title.svelte";

	let { children, actions }: { children: Snippet; actions?: Snippet } = $props();

	const team = $derived(currentTeam());
	const editable = $derived(writes(team?.role));
	const tabs = $derived(
		[
			{ label: "Monitors", href: resolve("/uptime"), all: true },
			{ label: "Status pages", href: resolve("/uptime/pages"), all: false },
			{ label: "Alerts", href: resolve("/uptime/alerts"), all: false },
			{ label: "Maintenance", href: resolve("/uptime/maintenance"), all: true }
		]
			.filter((tab) => tab.all || editable)
			.map((tab) => ({ ...tab, current: page.url.pathname === tab.href }))
	);
</script>

<svelte:head>
	<title>{tabs.find((tab) => tab.current && tab.label !== "Monitors")?.label ?? "Uptime"}</title>
</svelte:head>

<Shell>
	<div class="flex flex-col gap-6">
		<div class="flex flex-col gap-3">
			<div class="flex min-h-control items-center justify-between gap-4">
				<Title view="uptime-title">
					{#if team}
						{team.personal ? "Personal" : team.name}
					{:else}
						<Skeleton class="h-6 w-40" />
					{/if}
				</Title>
				{@render actions?.()}
			</div>
			<div
				class="flex h-10 overflow-x-auto border-b border-line [view-transition-name:uptime-tabs]"
			>
				<Nav label="uptime" items={tabs} />
			</div>
		</div>
		{@render children()}
	</div>
</Shell>
