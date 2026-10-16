<script lang="ts">
	import type { Snippet } from "svelte";
	import Icon from "./Icon.svelte";

	let {
		sections,
		title,
		children
	}: {
		sections: { id: string; label: string; icon?: string; group?: string }[];
		title: Snippet;
		children: Snippet;
	} = $props();

	let active = $state("");
	// a section picked by link or click stays in view and marked until the reader scrolls on their own
	let pinned = false;
	let content = $state<HTMLElement>();

	function hold() {
		if (pinned) document.getElementById(active)?.scrollIntoView({ block: "start" });
	}

	let arrived = false;
	$effect(() => {
		const id = decodeURIComponent(location.hash.slice(1));
		if (arrived || !sections.some((section) => section.id === id)) return;
		arrived = true;
		active = id;
		pinned = true;
		hold();
	});

	// sections above the picked one grow as their data loads
	$effect(() => {
		if (!content) return;
		const observer = new ResizeObserver(hold);
		observer.observe(content);
		return () => observer.disconnect();
	});

	function track() {
		if (pinned) return;
		const passed = sections.filter(
			(section) =>
				(document.getElementById(section.id)?.getBoundingClientRect().top ?? 1e9) < 120
		);
		// short sections at the end never reach the top, so the bottom of the page picks the last
		const bottom = innerHeight + scrollY >= document.documentElement.scrollHeight - 2;
		active = (bottom ? sections.at(-1) : passed.at(-1))?.id ?? sections[0]?.id ?? "";
	}

	$effect(() => {
		void sections;
		track();
	});
</script>

<svelte:window
	onscroll={track}
	onwheel={() => (pinned = false)}
	ontouchmove={() => (pinned = false)}
	onkeydown={() => (pinned = false)}
/>

<div class="flex flex-col gap-8">
	<div class="flex min-h-9 items-center gap-3">
		{@render title()}
	</div>
	<div class="grid grid-cols-1 items-start gap-8 lg:grid-cols-[11rem_minmax(0,1fr)]">
		<nav
			class="flex min-w-0 [scrollbar-width:none] gap-1 overflow-x-auto lg:sticky lg:top-6 lg:flex-col"
		>
			{#each sections as section, i (section.id)}
				{#if section.group !== sections[i - 1]?.group}
					<span
						class="hidden px-3 pb-1 label text-[10px] text-faint lg:block {i
							? 'pt-4'
							: ''}">{section.group ?? ""}</span
					>
				{/if}
				<a
					href="#{section.id}"
					onclick={() => {
						active = section.id;
						pinned = true;
					}}
					class="flex h-9 shrink-0 items-center gap-2.5 rounded-px px-3 label transition-colors duration-100 hover:bg-raised {section.id ===
					'danger'
						? 'text-bad'
						: 'hover:text-fg'} {active === section.id ? 'bg-raised' : ''} {active ===
						section.id && section.id !== 'danger'
						? 'text-fg'
						: ''}"
				>
					{#if section.icon}
						<Icon
							svg={section.icon}
							size={12}
							class={active === section.id && section.id !== "danger"
								? "text-cyan"
								: ""}
						/>
					{/if}
					{section.label}
				</a>
			{/each}
		</nav>
		<div class="flex max-w-3xl flex-col gap-6" bind:this={content}>
			{@render children()}
		</div>
	</div>
</div>
