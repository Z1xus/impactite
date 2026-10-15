<script lang="ts">
	import type { Snippet } from "svelte";

	let {
		sections,
		title,
		children
	}: { sections: { id: string; label: string }[]; title: Snippet; children: Snippet } = $props();

	let active = $state("");

	function track() {
		const passed = sections.filter(
			(section) =>
				(document.getElementById(section.id)?.getBoundingClientRect().top ?? 1e9) < 120
		);
		active = passed.at(-1)?.id ?? sections[0]?.id ?? "";
	}

	$effect(() => {
		void sections;
		track();
	});
</script>

<svelte:window onscroll={track} />

<div class="flex flex-col gap-8">
	<div class="flex min-h-9 items-center gap-3">
		{@render title()}
	</div>
	<div class="grid grid-cols-1 items-start gap-8 lg:grid-cols-[11rem_minmax(0,1fr)]">
		<nav
			class="flex min-w-0 [scrollbar-width:none] gap-1 overflow-x-auto lg:sticky lg:top-6 lg:flex-col"
		>
			{#each sections as section (section.id)}
				<a
					href="#{section.id}"
					class="flex h-9 shrink-0 items-center rounded-px px-3 label transition-colors duration-100 hover:bg-raised hover:text-fg {active ===
					section.id
						? 'bg-raised text-fg'
						: ''}">{section.label}</a
				>
			{/each}
		</nav>
		<div class="flex max-w-3xl flex-col gap-6">
			{@render children()}
		</div>
	</div>
</div>
