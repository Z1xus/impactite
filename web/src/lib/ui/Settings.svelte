<script lang="ts">
	import type { Snippet } from "svelte";
	import { page } from "$app/state";
	import { goto } from "$app/navigation";
	import { sentence } from "./text";
	import Icon from "./Icon.svelte";
	import Select from "./Select.svelte";

	let {
		sections,
		current,
		title,
		children
	}: {
		sections: { id: string; label: string; icon?: string; group?: string; href: string }[];
		current: string;
		title: Snippet;
		children: Snippet;
	} = $props();

	$effect(() => {
		if (page.params.section || !page.url.hash) return;
		const legacy = sections.find((item) => item.id === page.url.hash.slice(1));
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- section hrefs are resolved by callers
		if (legacy) goto(`${legacy.href}${page.url.search}`, { replaceState: true });
	});
</script>

<div class="flex flex-col gap-8">
	<div class="flex min-h-9 items-center gap-3">
		{@render title()}
	</div>
	<div class="grid grid-cols-1 items-start gap-8 lg:grid-cols-[11rem_minmax(0,1fr)]">
		<div class="lg:hidden">
			<Select
				label="settings"
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
				{#if section.group !== sections[i - 1]?.group}
					<span class="px-3 pb-1 label text-[10px] text-faint {i ? 'pt-4' : ''}"
						>{section.group ?? ""}</span
					>
				{/if}
				<!-- eslint-disable svelte/no-navigation-without-resolve -- hrefs come resolved from callers -->
				<a
					href={section.href}
					aria-current={current === section.id ? "page" : undefined}
					class="flex h-9 shrink-0 items-center gap-2.5 rounded-px px-3 font-sans text-[14px] transition-colors duration-100 hover:bg-raised {section.id ===
					'danger'
						? 'text-bad'
						: 'hover:text-fg'} {current === section.id ? 'bg-raised' : ''} {current ===
						section.id && section.id !== 'danger'
						? 'text-fg'
						: ''}"
				>
					{#if section.icon}
						<Icon
							svg={section.icon}
							size={12}
							class={current === section.id && section.id !== "danger"
								? "text-cyan"
								: ""}
						/>
					{/if}
					{sentence(section.label)}
				</a>
				<!-- eslint-enable svelte/no-navigation-without-resolve -->
			{/each}
		</nav>
		<div class="flex max-w-3xl flex-col gap-6">
			{@render children()}
		</div>
	</div>
</div>
