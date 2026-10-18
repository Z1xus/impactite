<script lang="ts">
	import { page } from "$app/state";
	import { resolve } from "$app/paths";
	import Danger from "#lib/account/Danger.svelte";
	import Display from "#lib/account/Display.svelte";
	import Keys from "#lib/account/Keys.svelte";
	import Passkeys from "#lib/account/Passkeys.svelte";
	import Password from "#lib/account/Password.svelte";
	import Profile from "#lib/account/Profile.svelte";
	import Sessions from "#lib/account/Sessions.svelte";
	import TwoFactor from "#lib/account/TwoFactor.svelte";
	import code from "pixelarticons/svg/code.svg?raw";
	import key from "pixelarticons/svg/key.svg?raw";
	import laptop from "pixelarticons/svg/laptop.svg?raw";
	import lock from "pixelarticons/svg/lock.svg?raw";
	import monitor from "pixelarticons/svg/monitor.svg?raw";
	import phone from "pixelarticons/svg/phone.svg?raw";
	import alert from "pixelarticons/svg/square-alert.svg?raw";
	import user from "pixelarticons/svg/user.svg?raw";
	import Missing from "#lib/ui/Missing.svelte";
	import Settings from "#lib/ui/Settings.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Title from "#lib/ui/Title.svelte";

	const section = $derived(page.params.section ?? "profile");
	const sections = $derived(
		[
			{ id: "profile", label: "profile", icon: user, group: "account" },
			{ id: "display", label: "display", icon: monitor, group: "account" },
			{ id: "password", label: "password", icon: lock, group: "sign in" },
			{ id: "two-factor", label: "two factor", icon: phone, group: "sign in" },
			{ id: "passkeys", label: "passkeys", icon: key, group: "sign in" },
			{ id: "sessions", label: "sessions", icon: laptop, group: "sign in" },
			{ id: "keys", label: "API keys", icon: code, group: "developers" },
			{ id: "danger", label: "danger", icon: alert, danger: true }
		].map((item) => ({ ...item, href: resolve("/settings/[[section]]", { section: item.id }) }))
	);
</script>

<Shell>
	{#if sections.some((item) => item.id === section)}
		<Settings {sections} current={section}>
			{#snippet title()}
				<Title>Account</Title>
			{/snippet}

			{#if section === "profile"}
				<Profile />
			{:else if section === "display"}
				<Display />
			{:else if section === "password"}
				<Password />
			{:else if section === "two-factor"}
				<TwoFactor />
			{:else if section === "passkeys"}
				<Passkeys />
			{:else if section === "sessions"}
				<Sessions />
			{:else if section === "keys"}
				<Keys />
			{:else if section === "danger"}
				<Danger />
			{/if}
		</Settings>
	{:else}
		<Missing text="This settings page doesn't exist." />
	{/if}
</Shell>
