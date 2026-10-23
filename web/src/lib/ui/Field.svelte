<script lang="ts">
	import type { Snippet } from "svelte";
	import type { HTMLInputAttributes } from "svelte/elements";

	let {
		label,
		value = $bindable(""),
		size = "md",
		rows,
		lead,
		invalid = false,
		quiet = false,
		unit,
		prefix,
		...rest
	}: Omit<HTMLInputAttributes, "size"> & {
		label: string;
		value?: string;
		size?: "md" | "sm";
		rows?: number;
		lead?: Snippet;
		invalid?: boolean;
		quiet?: boolean;
		unit?: string;
		prefix?: string;
	} = $props();

	// password managers only see account fields
	const credential = $derived(!!rest.autocomplete && rest.autocomplete !== "off");
	const quietly = $derived(
		credential
			? {}
			: {
					autocomplete: "off" as const,
					"data-1p-ignore": "",
					"data-lpignore": "true",
					"data-bwignore": "",
					"data-form-type": "other"
				}
	);

	const look = $derived(
		`rounded-px border bg-panel text-fg ${
			size === "sm" ? "h-compact px-2.5 text-small" : "px-3 text-body"
		} transition-colors duration-100 outline-none placeholder:text-faint ${
			invalid ? "border-bad" : "border-control hover:border-dim focus:border-cyan"
		}`
	);
</script>

<label class="flex flex-col gap-2">
	<span class={size === "sm" || quiet ? "sr-only" : "label leading-[14px]"}>{label}</span>
	{#if rows}
		<textarea
			{...rest as object}
			{...quietly}
			{rows}
			bind:value
			class="{look} resize-y py-2.5 font-code text-small leading-5"></textarea>
	{:else if prefix}
		<span
			class="flex h-control items-center rounded-px border border-control bg-panel transition-colors duration-100 focus-within:border-cyan hover:border-dim"
		>
			<span class="truncate pl-3 text-body text-faint">{prefix}</span>
			<input
				{...rest}
				{...quietly}
				bind:value
				class="h-full min-w-16 flex-1 bg-transparent pr-3 text-body text-fg outline-none placeholder:text-faint"
			/>
		</span>
	{:else if unit}
		<span class="relative flex items-center">
			<input {...rest} {...quietly} bind:value class="{look} h-control w-full pr-10" />
			<span class="pointer-events-none absolute right-3 meta">{unit}</span>
		</span>
	{:else if lead}
		<span class="relative flex items-center">
			<span class="pointer-events-none absolute left-2">{@render lead()}</span>
			<input {...rest} {...quietly} bind:value class="{look} h-control w-full pl-9" />
		</span>
	{:else}
		<input
			{...rest}
			{...quietly}
			bind:value
			class="{look} {size === 'sm' ? '' : 'h-control'}"
		/>
	{/if}
</label>
