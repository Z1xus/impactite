<script lang="ts">
	import { CHANNELS, channel, detect, type Channel } from "#lib/channels.ts";
	import Field from "./Field.svelte";
	import Glyph from "./Glyph.svelte";
	import Pick from "./Pick.svelte";

	let {
		label,
		value = $bindable(""),
		mail = true
	}: { label: string; value?: string; mail?: boolean } = $props();

	let input = $state<HTMLInputElement>();
	let hint = $state<Channel | null>(null);
	let touched = $state(false);

	const found = $derived(detect(value));
	const shown = $derived(channel(found.kind ?? hint ?? ""));
</script>

<div class="flex flex-col gap-2">
	<div class="flex flex-wrap items-center gap-1.5" role="group" aria-label="supported">
		{#each CHANNELS as item (item.kind)}
			<Pick
				spec={item.spec}
				label={item.name}
				on={found.kind === item.kind || (!found.kind && hint === item.kind)}
				dim={!!found.kind}
				title={item.kind === "email" && !mail
					? "Mail is not set up on this server"
					: item.example}
				onclick={() => {
					hint = item.kind;
					input?.focus();
				}}
			/>
		{/each}
		{#if touched && found.error}
			<span class="ml-auto text-small text-bad max-sm:w-full" role="alert">{found.error}</span
			>
		{/if}
	</div>
	<Field
		{label}
		quiet
		bind:value
		placeholder={channel(hint ?? "")?.example ?? "Paste a URL or enter an email"}
		autocomplete="off"
		autocapitalize="off"
		spellcheck={false}
		invalid={touched && !!found.error}
		onblur={() => (touched = !!value.trim())}
		oninput={() => (touched = false)}
		{@attach (node: HTMLInputElement) => {
			input = node;
		}}
		required
	>
		{#snippet lead()}
			{#if shown}
				<Glyph spec={shown.spec} />
			{:else}
				<span class="block h-5 w-5 rounded-[3px] border border-dashed border-edge"></span>
			{/if}
		{/snippet}
	</Field>
</div>
