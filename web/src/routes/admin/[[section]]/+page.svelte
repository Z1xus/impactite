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
	import { session } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import List from "#lib/ui/List.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Settings from "#lib/ui/Settings.svelte";
	import Title from "#lib/ui/Title.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";

	type User = {
		id: number;
		name: string;
		email: string;
		admin: boolean;
		created: number;
		sites: number;
	};
	type Admin = {
		signup: boolean;
		mail: boolean;
		users: User[];
		version: string;
		latest: {
			version: string;
			url: string;
			changes: { version: string; lines: string[] }[];
		} | null;
	};

	let info = $state<Admin | null>(null);

	$effect(() => {
		api<Admin>("/api/app/admin").then((result) => (info = result));
	});

	const update = (action: () => Promise<Admin | void>) =>
		attempt(async () => {
			info = (await action()) || info;
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
				<Section id="version" label="version">
					{#if info.latest}
						<div class="flex flex-col gap-2">
							<span class="text-[14px] text-fg">
								impactite {info.latest.version} is available. You're on {info.version}.
								<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- the release page on github -->
								<a href={info.latest.url} class="text-cyan hover:text-fg"
									>What changed</a
								>
							</span>
							{#each info.latest.changes as change (change.version)}
								<div class="flex flex-col gap-1">
									<span class="label text-dim">{change.version}</span>
									{#each change.lines as line, i (i)}
										{@const [kind, text] = line.includes(": ")
											? [
													line.slice(0, line.indexOf(": ")),
													line.slice(line.indexOf(": ") + 2)
												]
											: ["", line]}
										<span class="flex gap-2 text-[14px] text-fg">
											{#if kind}<span
													class="w-14 shrink-0 pt-0.5 label text-cyan"
													>{kind}</span
												>{/if}
											{text}
										</span>
									{/each}
								</div>
							{/each}
							<span class="text-[14px] text-dim">
								Run impactite update or pull the new Docker image, then restart.
							</span>
						</div>
					{:else}
						<span class="text-[14px] text-dim"
							>impactite {info.version} is up to date.</span
						>
					{/if}
				</Section>
			{/if}

			{#if section === "signup"}
				<Section id="signup" label="sign-up">
					<div class="flex items-center justify-between gap-4">
						<span class="text-[14px] text-dim">Who can create an account.</span>
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
