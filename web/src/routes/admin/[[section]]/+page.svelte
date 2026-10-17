<script lang="ts">
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import Missing from "#lib/ui/Missing.svelte";
	import server from "pixelarticons/svg/server.svg?raw";
	import userPlus from "pixelarticons/svg/user-plus.svg?raw";
	import mail from "pixelarticons/svg/mail.svg?raw";
	import users from "pixelarticons/svg/users.svg?raw";
	import { date } from "#lib/display.svelte.ts";
	import { ask } from "#lib/ask.svelte.ts";
	import { attempt, toast } from "#lib/toast.svelte.ts";
	import { api } from "#lib/api.ts";
	import type { Admin } from "#lib/admin/types.ts";
	import Version from "#lib/admin/Version.svelte";
	import { session } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import List from "#lib/ui/List.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Settings from "#lib/ui/Settings.svelte";
	import Title from "#lib/ui/Title.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import Note from "#lib/ui/Note.svelte";

	let info = $state<Admin | null>(null);
	let checking = $state(false);

	$effect(() => {
		api<Admin>("/api/app/admin").then((result) => (info = result));
	});

	const update = (action: () => Promise<Admin | void>) =>
		attempt(async () => {
			info = (await action()) || info;
		});

	const checkUpdate = () =>
		attempt(async () => {
			checking = true;
			try {
				info = await api<Admin>("/api/app/admin/update-check", {});
			} finally {
				checking = false;
			}
		});

	const setSignup = (mode: "invite" | "open") =>
		update(() => api<Admin>("/api/app/admin", { signup: mode === "open" }, "PATCH"));

	const testMail = () =>
		update(async () => {
			const { sent } = await api<{ sent: string }>("/api/app/admin/mail", {});
			toast(`Sent a test mail to ${sent}.`);
		});

	const setAdmin = (user: number, admin: boolean) =>
		update(() => api<Admin>(`/api/app/admin/users/${user}`, { admin }, "PATCH"));

	const remove = (user: string | number) =>
		update(async () => {
			const yes = await ask({
				title: "Delete this user?",
				body: "Sites only they own are deleted too.",
				action: "delete user"
			});
			if (yes) return api<Admin>(`/api/app/admin/users/${user}`, undefined, "DELETE");
		});
	const section = $derived(page.params.section ?? "version");
	const sections = $derived(
		[
			{ id: "version", label: "version", icon: server },
			{ id: "signup", label: "sign-up", icon: userPlus },
			{ id: "mail", label: "mail", icon: mail },
			{ id: "users", label: "users", icon: users }
		].map((item) => ({ ...item, href: resolve("/admin/[[section]]", { section: item.id }) }))
	);

	$effect(() => {
		if (section !== "version" || !info?.update.checking) return;
		const timer = setTimeout(() => {
			api<Admin>("/api/app/admin").then((result) => (info = result));
		}, 1000);
		return () => clearTimeout(timer);
	});
</script>

<Shell>
	{#if !sections.some((item) => item.id === section)}
		<Missing text="This settings page doesn't exist." />
	{:else if info}
		<Settings {sections} current={section}>
			{#snippet title()}
				<Title>Instance</Title>
			{/snippet}

			{#if section === "version"}
				<Version {info} {checking} check={checkUpdate} />
			{/if}

			{#if section === "signup"}
				<Section id="signup" label="sign-up">
					<div class="flex items-center justify-between gap-4">
						<Note>Who can create an account.</Note>
						<Toggle
							bind:value={
								() => (info?.signup ? "open" : "invite"),
								(mode: "invite" | "open") => setSignup(mode)
							}
							options={[
								{ key: "invite", label: "invite only" },
								{ key: "open", label: "open" }
							]}
						/>
					</div>
				</Section>
			{/if}

			{#if section === "mail"}
				<Section id="mail" label="mail">
					{#snippet actions()}
						{#if info?.mail}
							<Button variant="quiet" onclick={testMail}>Send test mail</Button>
						{/if}
					{/snippet}
					<span class="text-[14px] {info.mail ? 'text-fg' : 'text-dim'}">
						{info.mail
							? "Mail is set up."
							: "Mail is off. Start the server with --mail to send invites and sign-in links."}
					</span>
				</Section>
			{/if}

			{#if section === "users"}
				<Section id="users" label="users">
					<List
						items={info.users.map((user) => ({
							key: user.id,
							title: user.name,
							avatar: user.id,
							meta: [
								user.email,
								`${user.sites} ${user.sites === 1 ? "site" : "sites"}`,
								`since ${date(user.created)}`
							],
							removable: user.id !== session.me?.id
						}))}
						{remove}
					>
						{#snippet control(key)}
							{@const user = info?.users.find((user) => user.id === key)}
							{#if user && user.id !== session.me?.id}
								<Select
									align="right"
									value={user.admin ? "admin" : "user"}
									options={[
										{ key: "user", label: "user" },
										{ key: "admin", label: "admin" }
									]}
									pick={(role) => setAdmin(user.id, role === "admin")}
								/>
							{:else if user}
								<span class="label">admin</span>
							{/if}
						{/snippet}
					</List>
				</Section>
			{/if}
		</Settings>
	{/if}
</Shell>
