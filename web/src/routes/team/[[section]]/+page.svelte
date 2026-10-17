<script lang="ts">
	import { page } from "$app/state";
	import Missing from "#lib/ui/Missing.svelte";
	import sliders from "pixelarticons/svg/sliders.svg?raw";
	import users from "pixelarticons/svg/users.svg?raw";
	import alert from "pixelarticons/svg/square-alert.svg?raw";
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { ask } from "#lib/ask.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import { api } from "#lib/api.ts";
	import { currentTeam, refresh, session } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Code from "#lib/ui/Code.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Danger from "#lib/ui/Danger.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import List from "#lib/ui/List.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Settings from "#lib/ui/Settings.svelte";
	import Shell from "#lib/ui/Shell.svelte";
	import Title from "#lib/ui/Title.svelte";
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

	const ROLES: { key: Role; label: string }[] = [
		{ key: "admin", label: "admin" },
		{ key: "editor", label: "editor" },
		{ key: "viewer", label: "viewer" }
	];

	const current = $derived(currentTeam());
	const id = $derived(session.team);
	const role = $derived(current?.role ?? "");
	const manager = $derived(role === "owner" || role === "admin");
	const editable = (member: Member) =>
		manager && member.role !== "owner" && member.id !== session.me?.id;
	const section = $derived(page.params.section ?? (manager ? "general" : "members"));
	const sections = $derived(
		[
			...(manager ? [{ id: "general", label: "general", icon: sliders }] : []),
			{ id: "members", label: "members", icon: users },
			{ id: "danger", label: "danger", icon: alert }
		].map((item) => ({ ...item, href: resolve("/team/[[section]]", { section: item.id }) }))
	);

	let team = $state.raw<Team | null>(null);
	let name = $state("");
	let inviting = $state(false);
	let email = $state("");
	let inviteRole = $state<Role>("editor");
	let link = $state("");
	let typed = $state("");

	$effect(() => {
		if (current && (current.personal || current.id === 0)) goto(resolve("/"));
		else if (id) load();
	});

	async function load() {
		team = await api<Team>(`/api/app/teams/${id}`);
		name = team.name;
	}

	async function rename() {
		await api(`/api/app/teams/${id}`, { name }, "PATCH");
		await refresh();
	}

	async function send(to: string, as: string) {
		const result = await api<{ link: string; sent: boolean }>(`/api/app/teams/${id}/invites`, {
			email: to,
			role: as
		});
		link = result.sent ? "" : result.link;
		await load();
	}

	async function invite() {
		await send(email, inviteRole);
		email = "";
		if (!link) inviting = false;
	}

	const setRole = (user: number, next: string) =>
		attempt(async () => {
			if (next === "owner") {
				const yes = await ask({
					title: "Hand over this team?",
					body: "You become an admin.",
					action: "hand over"
				});
				if (!yes) return;
				await api(`/api/app/teams/${id}/owner`, { user });
				await refresh();
			} else {
				await api(`/api/app/teams/${id}/members/${user}/role`, { role: next }, "PATCH");
			}
			await load();
		});

	const remove = (key: string | number) =>
		attempt(
			async () => {
				const [kind, value] = [String(key)[0], String(key).slice(1)];
				const path = kind === "m" ? "members" : "invites";
				await api(`/api/app/teams/${id}/${path}/${value}`, undefined, "DELETE");
				await load();
			},
			{
				title: String(key)[0] === "m" ? "Remove this member?" : "Revoke this invite?",
				body: String(key)[0] === "m" ? "They lose access to the team's sites." : undefined,
				action: String(key)[0] === "m" ? "remove member" : "revoke invite"
			}
		);

	const resend = (invite: Invite) =>
		attempt(async () => {
			await api(`/api/app/teams/${id}/invites/${invite.id}`, undefined, "DELETE");
			await send(invite.email, invite.role);
			if (link) inviting = true;
		});

	const leave = () =>
		attempt(
			async () => {
				await api(`/api/app/teams/${id}/members/${session.me?.id}`, undefined, "DELETE");
				await refresh();
				await goto(resolve("/"));
			},
			{
				title: `Leave ${team?.name ?? "this team"}?`,
				body: "You'll lose access to its sites.",
				action: "leave team"
			}
		);

	async function destroy() {
		if (typed.trim().toLowerCase() !== team?.name.toLowerCase())
			throw new Error(`Type ${team?.name} to confirm.`);
		await api(`/api/app/teams/${id}`, undefined, "DELETE");
		await refresh();
		await goto(resolve("/"));
	}
</script>

<Dialog title="invite to {team?.name ?? 'team'}" bind:open={inviting}>
	<div class="flex flex-col gap-4">
		<Form action="send invite" submit={invite}>
			<Field
				label="email"
				type="email"
				placeholder="ada@example.com"
				bind:value={email}
				required
			/>
			<Toggle label="role" bind:value={inviteRole} options={ROLES} />
		</Form>
		{#if link}
			<Code code={link} />
			<Message>Mail is not set up, so share this link yourself.</Message>
		{/if}
	</div>
</Dialog>

<Shell>
	{#if !sections.some((item) => item.id === section)}
		<Missing text="This settings page doesn't exist." />
	{:else if team && !team.personal}
		<Settings {sections} current={section}>
			{#snippet title()}
				<Title>{team?.name}</Title>
			{/snippet}

			{#if manager && section === "general"}
				<Section id="general" label="general">
					<Form action="rename" submit={rename} inline>
						<Field label="name" bind:value={name} required />
					</Form>
				</Section>
			{/if}

			{#if section === "members"}
				<Section id="members" label="members">
					{#snippet actions()}
						{#if manager}
							<Button variant="quiet" onclick={() => ((link = ""), (inviting = true))}
								>Invite</Button
							>
						{/if}
					{/snippet}
					<List
						items={[
							...team.members.map((member) => ({
								key: `m${member.id}`,
								title: member.name,
								avatar: member.id,
								meta: editable(member)
									? [member.email]
									: [member.email, member.role],
								removable: editable(member)
							})),
							...(manager ? team.invites : []).map((invite) => ({
								key: `i${invite.id}`,
								title: invite.email,
								avatar: null,
								meta: ["invited", invite.role]
							}))
						]}
						{remove}
					>
						{#snippet control(key)}
							{@const member = team?.members.find(
								(member) => `m${member.id}` === key
							)}
							{@const invite = team?.invites.find(
								(invite) => `i${invite.id}` === key
							)}
							{#if member && editable(member)}
								<Select
									align="right"
									value={member.role}
									options={role === "owner"
										? [...ROLES, { key: "owner", label: "make owner" }]
										: ROLES}
									pick={(next) => setRole(member.id, next)}
								/>
							{:else if invite}
								<Button variant="plain" onclick={() => resend(invite)}
									>Resend</Button
								>
							{/if}
						{/snippet}
					</List>
				</Section>
			{/if}

			{#if section === "danger"}
				<Section id="danger" label="danger" danger>
					{#if role === "owner"}
						<Danger
							title="Delete team"
							body="This is permanent. You lose the team and all its sites."
						>
							<Form action="delete team" submit={destroy} inline danger>
								<Field
									label="type the team name to confirm"
									placeholder={team.name}
									bind:value={typed}
								/>
							</Form>
						</Danger>
					{:else}
						<Danger
							title="Leave team"
							body="You lose access to its sites until someone invites you again."
						>
							<div>
								<Button variant="danger" onclick={leave}>Leave {team.name}</Button>
							</div>
						</Danger>
					{/if}
				</Section>
			{/if}
		</Settings>
	{/if}
</Shell>
