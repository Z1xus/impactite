<script lang="ts">
	import qrcode from "qrcode-generator";
	import { api } from "#lib/api.ts";
	import { refresh, session } from "#lib/session.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Code from "#lib/ui/Code.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Note from "#lib/ui/Note.svelte";
	import Section from "#lib/ui/Section.svelte";

	let pending = $state<{ secret: string; uri: string } | null>(null);
	let code = $state("");
	let recovery = $state<string[] | null>(null);

	const qr = $derived.by(() => {
		if (!pending) return "";
		const image = qrcode(0, "M");
		image.addData(pending.uri);
		image.make();
		return image.createSvgTag({ cellSize: 4, margin: 0, scalable: true });
	});

	const start = () =>
		attempt(async () => {
			pending = await api("/api/app/totp/begin", {});
		});

	async function enable() {
		if (!pending) return;
		recovery = (
			await api<{ recovery: string[] }>("/api/app/totp/enable", {
				secret: pending.secret,
				code
			})
		).recovery;
		pending = null;
		code = "";
		await refresh();
	}

	async function disable() {
		await api("/api/app/totp/disable", { code });
		code = "";
		recovery = null;
		await refresh();
	}
</script>

<Section id="two-factor" label="two factor">
	{#snippet actions()}
		{#if !session.me?.totp && !pending}
			<Button variant="quiet" onclick={start}>Set up</Button>
		{/if}
	{/snippet}
	{#if recovery}
		<Note>
			Two factor is on. Save these recovery codes now. Each one lets you sign in once without
			the app.
		</Note>
		<Code code={recovery.join("\n")} />
	{:else if session.me?.totp}
		<Note>
			Password sign in asks for a code from your authenticator app. Passkeys need no code.
		</Note>
		<Form action="turn off" submit={disable} inline>
			<Field
				label="code from your app or a recovery code"
				bind:value={code}
				autocomplete="one-time-code"
				required
			/>
		</Form>
	{:else if pending}
		<div class="flex flex-wrap items-start gap-6">
			<!-- eslint-disable-next-line svelte/no-at-html-tags -- svg built from our own otpauth uri -->
			<div class="w-40 shrink-0 rounded-px bg-white p-2">{@html qr}</div>
			<div class="flex min-w-56 flex-1 flex-col gap-3">
				<Note>Scan this with your authenticator app, or enter the key by hand.</Note>
				<Code code={pending.secret} />
				<Form action="turn on" submit={enable} inline>
					<Field
						label="code from your app"
						bind:value={code}
						autocomplete="one-time-code"
						inputmode="numeric"
						required
					/>
				</Form>
			</div>
		</div>
	{:else}
		<Note>Ask for a code from an authenticator app when you sign in with a password.</Note>
	{/if}
</Section>
