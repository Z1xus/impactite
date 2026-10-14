<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { api } from "#lib/api.ts";
	import { currentTeam, pickTeam, refresh, session } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Code from "#lib/ui/Code.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";

	type Member = { id: number; name: string; email: string; role: string };
	type Invite = { id: number; email: string; role: string; expires: number };
	type Team = {
		id: number;
		name: string;
		personal: boolean;
		members: Member[];
		invites: Invite[];
	};
	type Role = "admin" | "editor" | "viewer";

	const ROLES: { key: string; label: string }[] = [
		{ key: "admin", label: "admin" },
		{ key: "editor", label: "editor" },
		{ key: "viewer", label: "viewer" }
	];

	const id = $derived(session.team);
	const role = $derived(currentTeam()?.role ?? "");
	const manager = $derived(role === "owner" || role === "admin");

	const editable = (member: Member) =>
		manager && member.role !== "owner" && member.id !== session.me?.id;

	let team = $state<Team | null>(null);
	let name = $state("");
	let email = $state("");
	let inviteRole = $state<Role>("editor");
	let link = $state("");
	let confirm = $state("");
	let newTeam = $state("");
	let error = $state("");

	async function load() {
		if (!id) return;
		team = await api<Team>(`/api/app/teams/${id}`);
		name = team.name;
	}

	$effect(() => {
		void id;
		link = "";
		load();
	});

	async function attempt(action: () => Promise<unknown>) {
		error = "";
		try {
			await action();
		} catch (err) {
			error = err instanceof Error ? err.message : "Something went wrong.";
		}
	}

	async function rename() {
		await api(`/api/app/teams/${id}`, { name }, "PATCH");
		await refresh();
	}

	async function invite() {
		const result = await api<{ link: string; sent: boolean }>(`/api/app/teams/${id}/invites`, {
			email,
			role: inviteRole
		});
		link = result.sent ? "" : result.link;
		email = "";
		await load();
	}

	const setRole = (user: number, next: string) =>
		attempt(async () => {
			if (next === "owner") {
				if (!window.confirm("Hand over this team? You become an admin.")) return;
				await api(`/api/app/teams/${id}/owner`, { user });
				await refresh();
			} else {
				await api(`/api/app/teams/${id}/members/${user}/role`, { role: next }, "PATCH");
			}
			await load();
		});

	const removeMember = (user: string | number) =>
		attempt(async () => {
			await api(`/api/app/teams/${id}/members/${user}`, undefined, "DELETE");
			await load();
		});

	const revoke = (invite: string | number) =>
		attempt(async () => {
			await api(`/api/app/teams/${id}/invites/${invite}`, undefined, "DELETE");
			await load();
		});

	const resend = (invite: Invite) =>
		attempt(async () => {
			await api(`/api/app/teams/${id}/invites/${invite.id}`, undefined, "DELETE");
			const result = await api<{ link: string; sent: boolean }>(
				`/api/app/teams/${id}/invites`,
				{
					email: invite.email,
					role: invite.role
				}
			);
			link = result.sent ? "" : result.link;
			await load();
		});

	const leave = () =>
		attempt(async () => {
			await api(`/api/app/teams/${id}/members/${session.me?.id}`, undefined, "DELETE");
			await refresh();
			await goto(resolve("/"));
		});

	async function destroy() {
		if (confirm !== team?.name) throw new Error(`Type ${team?.name} to confirm.`);
		await api(`/api/app/teams/${id}`, undefined, "DELETE");
		await refresh();
		await goto(resolve("/"));
	}

	async function create() {
		const created = await api<{ id: number }>("/api/app/teams", { name: newTeam });
		newTeam = "";
		await refresh();
		pickTeam(created.id);
	}
</script>

<Shell>
	<div class="flex flex-col gap-12">
		{#if team && !team.personal}
			<Section label="team">
				{#if manager}
					<Form action="rename" submit={rename} inline>
						<Field label="name" bind:value={name} required />
					</Form>
				{:else}
					<span class="text-fg">{team.name}</span>
				{/if}
			</Section>

			<Section label="members">
				<List
					items={team.members.map((member) => ({
						key: member.id,
						title: member.name,
						meta: editable(member) ? [member.email] : [member.email, member.role],
						removable: editable(member)
					}))}
					remove={removeMember}
				>
					{#snippet control(key)}
						{@const member = team?.members.find((member) => member.id === key)}
						{#if member && editable(member)}
							<Select
								value={member.role}
								options={role === "owner"
									? [...ROLES, { key: "owner", label: "make owner" }]
									: ROLES}
								pick={(next) => setRole(member.id, next)}
							/>
						{/if}
					{/snippet}
				</List>
			</Section>

			{#if manager}
				<Section label="invites">
					{#if team.invites.length}
						<List
							items={team.invites.map((invite) => ({
								key: invite.id,
								title: invite.email,
								meta: [invite.role]
							}))}
							remove={revoke}
						>
							{#snippet control(key)}
								{@const invite = team?.invites.find((invite) => invite.id === key)}
								{#if invite}
									<Button variant="plain" onclick={() => resend(invite)}
										>resend</Button
									>
								{/if}
							{/snippet}
						</List>
					{/if}
					<Form action="invite" submit={invite} inline>
						<Field
							label="email"
							type="email"
							placeholder="ada@example.com"
							bind:value={email}
							required
						/>
						<Toggle
							bind:value={inviteRole}
							options={ROLES as { key: Role; label: string }[]}
						/>
					</Form>
					{#if link}
						<Code code={link} />
						<Message>Mail is not set up, so share this link yourself.</Message>
					{/if}
				</Section>
			{/if}

			{#if role === "owner"}
				<Section label="delete team">
					<Form action="delete" submit={destroy} inline>
						<Field
							label="type {team.name} to delete it and all its sites"
							bind:value={confirm}
						/>
					</Form>
				</Section>
			{:else}
				<Section label="leave team">
					<div class="w-fit">
						<Button variant="quiet" onclick={leave}>leave {team.name}</Button>
					</div>
				</Section>
			{/if}
		{/if}

		<Section label="new team">
			<Form action="create team" submit={create} inline>
				<Field label="name" placeholder="Acme" bind:value={newTeam} required />
			</Form>
		</Section>

		<Message tone="bad">{error}</Message>
	</div>
</Shell>
