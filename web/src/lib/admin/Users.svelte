<script lang="ts">
	import { api } from "#lib/api.ts";
	import { date } from "#lib/display.svelte.ts";
	import { session } from "#lib/session.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import List from "#lib/ui/List.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Select from "#lib/ui/Select.svelte";
	import { admin } from "./admin.svelte.ts";
	import type { Admin } from "./types.ts";

	let { info }: { info: Admin } = $props();

	const setAdmin = (user: number, on: boolean) =>
		attempt(async () => {
			admin.info = await api<Admin>(`/api/app/admin/users/${user}`, { admin: on }, "PATCH");
		});

	const remove = (user: number) =>
		attempt(
			async () => {
				admin.info = await api<Admin>(`/api/app/admin/users/${user}`, undefined, "DELETE");
			},
			{
				title: "Delete this user?",
				body: "Their account and any sites only they own will be deleted. You can't undo this action.",
				action: "Delete user"
			}
		);
</script>

<Section id="users" label="users">
	<List
		items={info.users.map((user) => ({
			key: user.id,
			title: user.name,
			avatar: "joined",
			meta: [
				user.email,
				`${user.sites} ${user.sites === 1 ? "site" : "sites"}`,
				`since ${date(user.created)}`
			],
			removable: user.id !== session.me?.id
		}))}
		{remove}
	>
		{#snippet control(id)}
			{@const user = info.users.find((user) => user.id === id)}
			{#if user && user.id !== session.me?.id}
				<Select
					size="sm"
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
