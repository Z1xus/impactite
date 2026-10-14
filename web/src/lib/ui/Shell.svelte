<script lang="ts">
	import type { Snippet } from "svelte";
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import logout from "pixelarticons/svg/logout.svg?raw";
	import settings from "pixelarticons/svg/settings-cog.svg?raw";
	import sliders from "pixelarticons/svg/sliders.svg?raw";
	import user from "pixelarticons/svg/user.svg?raw";
	import users from "pixelarticons/svg/users.svg?raw";
	import { currentTeam } from "#lib/session.svelte.ts";
	import { api } from "#lib/api.ts";
	import { adopt, pickTeam, session } from "#lib/session.svelte.ts";
	import Icon from "./Icon.svelte";
	import Select from "./Select.svelte";

	let { children, actions }: { children: Snippet; actions?: Snippet } = $props();

	const teams = $derived(
		(session.me?.teams ?? []).map((team) => ({
			key: team.id,
			label: team.personal ? "personal" : team.name
		}))
	);

	async function signOut() {
		await api("/api/app/logout", {});
		adopt(null);
		await goto(resolve("/login"));
	}
</script>

<div class="min-h-dvh">
	<header class="border-b border-line">
		<div class="mx-auto flex h-14 max-w-6xl items-center gap-4 px-6">
			<a href={resolve("/")} class="font-mono text-[15px] text-fg">impactite</a>
			{#if session.team !== null}
				<Select
					value={session.team}
					options={teams}
					pick={pickTeam}
					icon={currentTeam()?.personal ? user : users}
				/>
				{#if session.team !== 0}
					<a
						href={resolve("/team")}
						class="grid h-8 w-8 place-items-center rounded-px text-dim transition-colors duration-100 hover:text-fg"
						aria-label="team settings"
					>
						<Icon svg={sliders} size={14} />
					</a>
				{/if}
			{/if}
			<div class="ml-auto flex items-center gap-3">
				{@render actions?.()}
				<a
					href={resolve("/settings")}
					class="grid h-8 w-8 place-items-center rounded-px text-dim transition-colors duration-100 hover:text-fg"
					aria-label="settings"
				>
					<Icon svg={settings} size={16} />
				</a>
				<button
					type="button"
					class="grid h-8 w-8 cursor-pointer place-items-center rounded-px text-dim transition-colors duration-100 hover:text-fg"
					aria-label="sign out"
					onclick={signOut}
				>
					<Icon svg={logout} size={16} />
				</button>
			</div>
		</div>
	</header>
	<main class="mx-auto max-w-6xl px-6 py-8">
		{@render children()}
	</main>
</div>
