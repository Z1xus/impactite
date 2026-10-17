<script lang="ts">
	import { display, saveDisplay } from "#lib/display.svelte.ts";
	import Section from "#lib/ui/Section.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
</script>

<Section id="display" label="display">
	<div class="flex flex-wrap gap-4">
		<Toggle
			label="theme"
			bind:value={() => display.theme, (theme) => saveDisplay({ theme })}
			options={[
				{ key: "dark", label: "dark" },
				{ key: "light", label: "light" },
				{ key: "system", label: "system" }
			]}
		/>
		<Toggle
			label="week starts on"
			bind:value={() => display.week, (week) => saveDisplay({ week })}
			options={[
				{ key: "monday", label: "monday" },
				{ key: "sunday", label: "sunday" }
			]}
		/>
		<Toggle
			label="clock"
			bind:value={() => display.clock, (clock) => saveDisplay({ clock })}
			options={[
				{ key: "24", label: "24 hour" },
				{ key: "12", label: "12 hour" }
			]}
		/>
		<Toggle
			label="numbers"
			bind:value={() => display.numbers, (numbers) => saveDisplay({ numbers })}
			options={[
				{ key: "en-US", label: "1,234.5" },
				{ key: "de-DE", label: "1.234,5" },
				{ key: "fr-FR", label: "1 234,5" }
			]}
		/>
	</div>
</Section>
