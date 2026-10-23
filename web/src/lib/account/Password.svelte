<script lang="ts">
	import { api } from "#lib/api.ts";
	import { refresh, session } from "#lib/session.svelte.ts";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Section from "#lib/ui/Section.svelte";
	import Username from "#lib/ui/Username.svelte";

	let current = $state("");
	let next = $state("");

	async function save() {
		await api("/api/app/me/password", { current, password: next }, "PUT");
		await refresh();
		current = next = "";
	}
</script>

<Section id="password" label="password">
	<Form
		action={session.me?.password ? "Change password" : "Set password"}
		done="Saved."
		submit={save}
	>
		<Username value={session.me?.email ?? ""} />
		<div class="grid gap-4 sm:grid-cols-2">
			{#if session.me?.password}
				<Field
					label="current password"
					type="password"
					bind:value={current}
					autocomplete="current-password"
					required
				/>
			{/if}
			<Field
				label="new password"
				type="password"
				bind:value={next}
				autocomplete="new-password"
				minlength={10}
				required
			/>
		</div>
	</Form>
</Section>
