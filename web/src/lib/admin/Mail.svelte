<script lang="ts">
	import { api } from "#lib/api.ts";
	import { attempt, toast } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Note from "#lib/ui/Note.svelte";
	import Section from "#lib/ui/Section.svelte";
	import type { Admin } from "./types.ts";

	let { info }: { info: Admin } = $props();

	const test = () =>
		attempt(async () => {
			const { sent } = await api<{ sent: string }>("/api/app/admin/mail", {});
			toast(`We sent a test email to ${sent}.`);
		});
</script>

<Section id="mail" label="mail">
	{#snippet actions()}
		{#if info.mail}
			<Button variant="quiet" onclick={test}>Send test mail</Button>
		{/if}
	{/snippet}
	{#if info.mail}
		<span class="text-body text-fg">Mail is on.</span>
	{:else}
		<Note>Mail is off. Start the server with --mail to send invites and sign-in links.</Note>
	{/if}
</Section>
