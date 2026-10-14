<script lang="ts">
	import { api } from "#lib/api.ts";
	import { session } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import List from "#lib/ui/List.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";

	type User = {
		id: number;
		name: string;
		email: string;
		admin: boolean;
		created: number;
		sites: number;
	};
	type Admin = { signup: boolean; mail: boolean; users: User[] };

	let info = $state<Admin | null>(null);
	let note = $state("");
	let error = $state("");

	const date = (seconds: number) =>
		new Date(seconds * 1000).toLocaleDateString("en-US", {
			month: "short",
			day: "numeric",
			year: "numeric"
		});

	$effect(() => {
		api<Admin>("/api/app/admin").then((result) => (info = result));
	});

	async function attempt(action: () => Promise<Admin | void>) {
		error = note = "";
		try {
			info = (await action()) || info;
		} catch (err) {
			error = err instanceof Error ? err.message : "Something went wrong.";
		}
	}

	const setSignup = (mode: "invite" | "open") =>
		attempt(() => api<Admin>("/api/app/admin", { signup: mode === "open" }, "PATCH"));

	const testMail = () =>
		attempt(async () => {
			const { sent } = await api<{ sent: string }>("/api/app/admin/mail", {});
			note = `Sent a test mail to ${sent}.`;
		});

	const setAdmin = (user: number, admin: boolean) =>
		attempt(() => api<Admin>(`/api/app/admin/users/${user}`, { admin }, "PATCH"));

	const remove = (user: string | number) =>
		attempt(() => {
			if (!window.confirm("Delete this user and the sites only they own?"))
				return Promise.resolve();
			return api<Admin>(`/api/app/admin/users/${user}`, undefined, "DELETE");
		});
</script>

<Shell>
	{#if info}
		<div class="flex flex-col gap-12">
			<Section label="sign-up">
				<div class="w-fit">
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

			<Section label="mail">
				<Message tone={info.mail ? "cyan" : "dim"}>
					{info.mail
						? "Mail is set up."
						: "Mail is off. Start the server with --mail to send invites and sign-in links."}
				</Message>
				{#if info.mail}
					<div class="w-fit">
						<Button variant="quiet" onclick={testMail}>send me a test mail</Button>
					</div>
				{/if}
			</Section>

			<Section label="users">
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

			<Message tone="cyan">{note}</Message>
			<Message tone="bad">{error}</Message>
		</div>
	{/if}
</Shell>
