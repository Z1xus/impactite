<script lang="ts">
	import type { Snippet } from "svelte";
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import logout from "pixelarticons/svg/logout.svg?raw";
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
				<Select value={session.team} options={teams} pick={pickTeam} />
			{/if}
			<div class="ml-auto flex items-center gap-3">
				{@render actions?.()}
				<button
					type="button"
					class="grid h-8 w-8 place-items-center rounded-px text-dim transition-colors duration-100 hover:text-fg"
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
