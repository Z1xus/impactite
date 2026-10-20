<script lang="ts">
	import { api } from "#lib/api.ts";
	import { display, saveDisplay, type Display } from "#lib/display.svelte.ts";
	import { session } from "#lib/session.svelte.ts";
	import Section from "#lib/ui/Section.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";

	function save(next: Partial<Display>) {
		saveDisplay(next);
		if (session.me) return api("/api/app/prefs/display", { ...display }, "PUT");
	}
</script>

<Section id="theme" label="theme">
	<div class="flex flex-wrap gap-4">
		<Toggle
			label="style"
			bind:value={() => display.style, (style) => save({ style })}
			options={[
				{ key: "pixel", label: "pixel" },
				{ key: "plain", label: "plain" }
			]}
		/>
		<Toggle
			label="mode"
			bind:value={() => display.theme, (theme) => save({ theme })}
			options={[
				{ key: "dark", label: "dark" },
				{ key: "light", label: "light" },
				{ key: "system", label: "system" }
			]}
		/>
	</div>
</Section>

<Section id="display" label="display">
	<div class="flex flex-wrap gap-4">
		<Toggle
			label="week starts on"
			bind:value={() => display.week, (week) => save({ week })}
			options={[
				{ key: "monday", label: "monday" },
				{ key: "sunday", label: "sunday" }
			]}
		/>
		<Toggle
			label="clock"
			bind:value={() => display.clock, (clock) => save({ clock })}
			options={[
				{ key: "24", label: "24 hour" },
				{ key: "12", label: "12 hour" }
			]}
		/>
		<Toggle
			label="numbers"
			bind:value={() => display.numbers, (numbers) => save({ numbers })}
			options={[
				{ key: "en-US", label: "1,234.5" },
				{ key: "de-DE", label: "1.234,5" },
				{ key: "fr-FR", label: "1 234,5" }
			]}
		/>
		{#if session.me}
			<Toggle
				label="all sites"
				bind:value={() => display.all, (all) => save({ all })}
				options={[
					{ key: "show", label: "show" },
					{ key: "hide", label: "hide" }
				]}
			/>
		{/if}
	</div>
</Section>
