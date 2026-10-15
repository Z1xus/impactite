<script lang="ts">
	import { goto } from "$app/navigation";
	import { resolve } from "$app/paths";
	import { page } from "$app/state";
	import { api, type Me } from "#lib/api.ts";
	import { create } from "#lib/passkey.ts";
	import { adopt, pickTeam, session } from "#lib/session.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Gate from "#lib/ui/Gate.svelte";
	import Message from "#lib/ui/Message.svelte";

	const token = page.params.token ?? "";
	let usePassword = $state(false);
	let name = $state("");
	let password = $state("");
	let place = $state("");
	let failed = $state("");

	$effect(() => {
		api<{ place: string }>(`/api/app/invites/${token}`)
			.then((invite) => (place = invite.place))
			.catch((err) => (failed = err.message));
	});

	async function submit() {
		const body: Record<string, unknown> = { token };
		if (!session.me) {
			body.name = name;
			if (usePassword) {
				body.password = password;
			} else {
				const { challenge, options } = await api<{ challenge: string; options: never }>(
					"/api/app/passkeys/signup",
					{ token, name, invite: true }
				);
				body.passkey = { challenge, credential: await create(options) };
			}
		}
		const { me, team } = await api<{ me: Me; team: number }>("/api/app/invites/accept", body);
		adopt(me);
		pickTeam(team);
		await goto(resolve("/"));
	}
</script>

<Gate>
	{#if failed}
		<Message tone="bad">{failed}</Message>
	{:else if place}
		<div class="flex flex-col gap-3">
			<p class="mb-3 text-center text-[14px] text-dim">
				You're invited to <span class="text-fg">{place}</span>.
			</p>
			<Form block action={session.me || usePassword ? "join" : "join with passkey"} {submit}>
				{#if !session.me}
					<Field label="name" bind:value={name} autocomplete="name" required />
					{#if usePassword}
						<Field
							label="password"
							type="password"
							bind:value={password}
							autocomplete="new-password"
							minlength={10}
							required
						/>
					{/if}
				{/if}
			</Form>
			{#if !session.me}
				<Button variant="plain" type="button" onclick={() => (usePassword = !usePassword)}>
					{usePassword ? "use a passkey" : "use a password instead"}
				</Button>
			{/if}
		</div>
	{/if}
</Gate>
