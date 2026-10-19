<script lang="ts">
	import type { Snippet } from "svelte";
	import { goto } from "$app/navigation";
	import { sentence } from "./text";
	import Empty from "./Empty.svelte";
	import Icon from "./Icon.svelte";
	import Section from "./Section.svelte";
	import Select from "./Select.svelte";
	import Skeleton from "./Skeleton.svelte";

	let {
		context,
		sections,
		current,
		title,
		loading = false,
		retry,
		children
	}: {
		context: string;
		sections: {
			id: string;
			label: string;
			icon?: string;
			group?: string;
			href: string;
			danger?: boolean;
		}[];
		current: string;
		title: Snippet;
		loading?: boolean;
		retry?: () => unknown;
		children: Snippet;
	} = $props();

	const label = $derived(sections.find((section) => section.id === current)?.label ?? "");
</script>

<svelte:head>
	<title>{sentence(label)} · {context} · impactite</title>
</svelte:head>

<div class="flex flex-col gap-8">
	<div class="flex min-h-control items-center gap-3">
		{@render title()}
	</div>
	<div class="grid grid-cols-1 items-start gap-8 lg:grid-cols-[11rem_minmax(0,1fr)]">
		<div class="lg:hidden">
			<Select
				value={current}
				options={sections.map((section) => ({
					key: section.id,
					label: sentence(section.label)
				}))}
				pick={(id) => {
					const href = sections.find((section) => section.id === id)?.href;
					// eslint-disable-next-line svelte/no-navigation-without-resolve -- hrefs come resolved from callers
					if (href) goto(href);
				}}
			/>
		</div>
		<nav aria-label="settings" class="hidden min-w-0 flex-col gap-1 lg:sticky lg:top-6 lg:flex">
			{#each sections as section, i (section.id)}
				{@const active = current === section.id}
				{#if section.group !== sections[i - 1]?.group}
					<span class="px-3 pb-1 label text-faint {i ? 'mt-4' : ''}"
						>{section.group ?? ""}</span
					>
				{/if}
				<!-- eslint-disable svelte/no-navigation-without-resolve -- hrefs come resolved from callers -->
				<a
					href={section.href}
					aria-current={active ? "page" : undefined}
					class="flex h-control shrink-0 items-center gap-2.5 rounded-px px-3 font-sans text-body hover:bg-raised {section.danger
						? 'text-bad'
						: active
							? 'text-cyan'
							: 'hover:text-fg'}"
				>
					{#if section.icon}
						<Icon svg={section.icon} size={12} />
					{/if}
					{sentence(section.label)}
				</a>
				<!-- eslint-enable svelte/no-navigation-without-resolve -->
			{/each}
		</nav>
		<div class="flex flex-col gap-6">
			{#if retry}
				<Section {label}>
					<Empty compact {retry} />
				</Section>
			{:else if loading}
				<Section {label}>
					<Skeleton class="h-control w-full" />
				</Section>
			{:else}
				{@render children()}
			{/if}
		</div>
	</div>
</div>
