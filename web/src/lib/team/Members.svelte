<script lang="ts">
	import resendIcon from "pixelarticons/svg/reload.svg?raw";
	import { api } from "#lib/api.ts";
	import { ask } from "#lib/ask.svelte.ts";
	import { date } from "#lib/display.svelte.ts";
	import { refresh, session } from "#lib/session.svelte.ts";
	import { attempt, toast } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Code from "#lib/ui/Code.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import List from "#lib/ui/List.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import { ROLES, type Invite, type Member, type Role, type Team } from "./types.ts";

	let {
		team,
		owner,
		manager,
		reload
	}: { team: Team; owner: boolean; manager: boolean; reload: () => Promise<void> } = $props();

	const base = $derived(`/api/app/teams/${team.id}`);
	const editable = (member: Member) =>
		manager && member.role !== "owner" && member.id !== session.me?.id;

	let inviting = $state(false);
	let email = $state("");
	let inviteRole = $state<Role>("editor");
	let link = $state("");

	async function send(to: string, as: string) {
		const result = await api<{ link: string; sent: boolean }>(`${base}/invites`, {
			email: to,
			role: as
		});
		link = result.sent ? "" : result.link;
		if (result.sent) toast("Invite sent.");
		await reload();
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
					body: "They will become the owner, and you'll become an admin.",
					action: "Hand over"
				});
				if (!yes) return;
				await api(`${base}/owner`, { user });
				await refresh();
			} else {
				await api(`${base}/members/${user}/role`, { role: next }, "PATCH");
			}
			await reload();
		});

	const removeMember = (user: number) =>
		attempt(
			async () => {
				await api(`${base}/members/${user}`, undefined, "DELETE");
				await reload();
			},
			{
				title: "Remove this member?",
				body: "They will no longer be able to see this team's sites.",
				action: "Remove member"
			}
		);

	const revoke = (id: number) =>
		attempt(
			async () => {
				await api(`${base}/invites/${id}`, undefined, "DELETE");
				await reload();
			},
			{
				title: "Revoke this invite?",
				body: "The invite link will stop working.",
				action: "Revoke invite"
			}
		);

	const resend = (pending: Invite) =>
		attempt(async () => {
			await api(`${base}/invites/${pending.id}`, undefined, "DELETE");
			await send(pending.email, pending.role);
			if (link) inviting = true;
		});
</script>

<Dialog title="invite to {team.name}" bind:open={inviting}>
	<div class="flex flex-col gap-4">
		<Form action="Invite" submit={invite}>
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
			<Message>Mail isn't set up, so you'll need to share this link yourself.</Message>
		{/if}
	</div>
</Dialog>

<Section id="members" label="members">
	{#snippet actions()}
		{#if manager}
			<Button variant="quiet" onclick={() => ((link = ""), (inviting = true))}>Invite</Button>
		{/if}
	{/snippet}
	<List
		items={team.members.map((member) => ({
			key: member.id,
			title: member.name,
			avatar: "joined",
			meta: editable(member) ? [member.email] : [member.email, member.role],
			removable: editable(member)
		}))}
		remove={removeMember}
		verb="Remove"
	>
		{#snippet control(id)}
			{@const member = team.members.find((member) => member.id === id)}
			{#if member && editable(member)}
				<Select
					size="sm"
					align="right"
					value={member.role}
					options={owner ? [...ROLES, { key: "owner", label: "make owner" }] : ROLES}
					pick={(next) => setRole(member.id, next)}
				/>
			{/if}
		{/snippet}
	</List>
</Section>

{#if manager && team.invites.length}
	<Section id="invites" label="invites">
		<List
			items={team.invites.map((pending) => ({
				key: pending.id,
				title: pending.email,
				avatar: "invited",
				meta: [pending.role, `expires ${date(pending.expires)}`]
			}))}
			remove={revoke}
			verb="Revoke"
		>
			{#snippet control(id)}
				{@const pending = team.invites.find((pending) => pending.id === id)}
				{#if pending}
					<Button
						variant="tool"
						aria-label="Resend invite to {pending.email}"
						title="Resend"
						onclick={() => resend(pending)}
					>
						<Icon svg={resendIcon} size={12} />
					</Button>
				{/if}
			{/snippet}
		</List>
	</Section>
{/if}
