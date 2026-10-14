<script lang="ts">
	import close from "pixelarticons/svg/close.svg?raw";
	import { api } from "#lib/api.ts";
	import { create } from "#lib/passkey.ts";
	import Form from "#lib/ui/Form.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Message from "#lib/ui/Message.svelte";
	import Shell from "#lib/ui/Shell.svelte";

	type Passkey = { id: string; name: string; created_at: number; used_at: number | null };

	let passkeys = $state<Passkey[] | null>(null);
	let error = $state("");

	const date = (seconds: number) =>
		new Date(seconds * 1000).toLocaleDateString("en-US", {
			month: "short",
			day: "numeric",
			year: "numeric"
		});

	$effect(() => {
		api<{ passkeys: Passkey[] }>("/api/app/passkeys").then((result) => {
			passkeys = result.passkeys;
		});
	});

	async function add() {
		const { challenge, options } = await api<{ challenge: string; options: never }>(
			"/api/app/passkeys/new",
			{}
		);
		const credential = await create(options);
		passkeys = (
			await api<{ passkeys: Passkey[] }>("/api/app/passkeys", { challenge, credential })
		).passkeys;
	}

	async function remove(id: string) {
		error = "";
		try {
			passkeys = (
				await api<{ passkeys: Passkey[] }>(
					`/api/app/passkeys/${encodeURIComponent(id)}`,
					undefined,
					"DELETE"
				)
			).passkeys;
		} catch (err) {
			error = err instanceof Error ? err.message : "Something went wrong.";
		}
	}
</script>

<Shell>
	<section class="flex max-w-xl flex-col gap-5">
		<span class="label">passkeys</span>
		<ul class="flex flex-col rounded-px border border-line bg-panel">
			{#if passkeys === null}
				<li class="h-14"></li>
			{:else}
				{#each passkeys as passkey (passkey.id)}
					<li
						class="flex items-center justify-between gap-4 border-b border-line px-4 py-3 last:border-0"
					>
						<div class="flex flex-col gap-1.5">
							<span class="text-[14px] text-fg">{passkey.name}</span>
							<span class="label">
								added {date(passkey.created_at)}{passkey.used_at
									? ` · used ${date(passkey.used_at)}`
									: ""}
							</span>
						</div>
						<button
							type="button"
							class="grid h-8 w-8 cursor-pointer place-items-center text-dim transition-colors duration-100 hover:text-bad"
							aria-label="remove passkey"
							onclick={() => remove(passkey.id)}
						>
							<Icon svg={close} size={14} />
						</button>
					</li>
				{/each}
			{/if}
		</ul>
		<Message tone="bad">{error}</Message>
		<div class="max-w-60">
			<Form action="add passkey" submit={add} />
		</div>
	</section>
</Shell>
