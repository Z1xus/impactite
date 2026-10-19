<script lang="ts">
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import mail from "pixelarticons/svg/mail.svg?raw";
	import server from "pixelarticons/svg/server.svg?raw";
	import userPlus from "pixelarticons/svg/user-plus.svg?raw";
	import users from "pixelarticons/svg/users.svg?raw";
	import { admin, load } from "#lib/admin/admin.svelte.ts";
	import Mail from "#lib/admin/Mail.svelte";
	import Signup from "#lib/admin/Signup.svelte";
	import Users from "#lib/admin/Users.svelte";
	import Version from "#lib/admin/Version.svelte";
	import { session } from "#lib/session.svelte.ts";
	import Missing from "#lib/ui/Missing.svelte";
	import Settings from "#lib/ui/Settings.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Title from "#lib/ui/Title.svelte";

	const section = $derived(page.params.section ?? "version");
	const sections = $derived(
		[
			{ id: "version", label: "version", icon: server },
			{ id: "signup", label: "sign up", icon: userPlus },
			{ id: "mail", label: "mail", icon: mail },
			{ id: "users", label: "users", icon: users }
		].map((item) => ({ ...item, href: resolve("/admin/[[section]]", { section: item.id }) }))
	);

	$effect(() => {
		if (session.me?.admin) load();
	});
</script>

<Shell>
	{#if !sections.some((item) => item.id === section)}
		<Missing text="This settings page doesn't exist." />
	{:else}
		<Settings
			context="Instance"
			{sections}
			current={section}
			loading={!admin.info}
			retry={admin.failed ? load : undefined}
		>
			{#snippet title()}
				<Title>Instance</Title>
			{/snippet}

			{#if admin.info}
				{#if section === "version"}
					<Version info={admin.info} />
				{:else if section === "signup"}
					<Signup info={admin.info} />
				{:else if section === "mail"}
					<Mail info={admin.info} />
				{:else}
					<Users info={admin.info} />
				{/if}
			{/if}
		</Settings>
	{/if}
</Shell>
