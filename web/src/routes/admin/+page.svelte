<script lang="ts">
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
		latest: { version: string; url: string } | null;
	};

	let info = $state<Admin | null>(null);

	const date = (seconds: number) =>
		new Date(seconds * 1000).toLocaleDateString("en-US", {
			month: "short",
			day: "numeric",
			year: "numeric"
		});

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
		update(() => {
			if (!window.confirm("Delete this user and the sites only they own?"))
				return Promise.resolve();
			return api<Admin>(`/api/app/admin/users/${user}`, undefined, "DELETE");
		});
	const sections = [
		{ id: "version", label: "version" },
		{ id: "signup", label: "sign-up" },
		{ id: "mail", label: "mail" },
		{ id: "users", label: "users" }
	];
</script>

<Shell>
	{#if info}
		<Settings {sections}>
			{#snippet title()}
				<Title>Instance</Title>
			{/snippet}

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
						<span class="text-[14px] text-dim">
							Run impactite update or pull the new Docker image, then restart.
						</span>
					</div>
				{:else}
					<span class="text-[14px] text-dim">impactite {info.version} is up to date.</span
					>
				{/if}
			</Section>

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

			<Section id="mail" label="mail">
				{#snippet actions()}
					{#if info?.mail}
						<Button variant="quiet" onclick={testMail}>send test mail</Button>
					{/if}
				{/snippet}
				<span class="text-[14px] {info.mail ? 'text-fg' : 'text-dim'}">
					{info.mail
						? "Mail is set up."
						: "Mail is off. Start the server with --mail to send invites and sign-in links."}
				</span>
			</Section>

			<Section id="users" label="users">
				<List
					items={info.users.map((user) => ({
						key: user.id,
						title: user.name,
						meta: [user.email, `${user.sites} sites`, `since ${date(user.created)}`],
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
		</Settings>
	{/if}
</Shell>
