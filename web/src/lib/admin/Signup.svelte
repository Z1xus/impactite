<script lang="ts">
	import { api } from "#lib/api.ts";
	import { attempt, toast } from "#lib/toast.svelte.ts";
	import Section from "#lib/ui/Section.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import { admin } from "./admin.svelte.ts";
	import type { Admin } from "./types.ts";

	let { info }: { info: Admin } = $props();

	const set = (mode: "invite" | "open") =>
		attempt(async () => {
			admin.info = await api<Admin>("/api/app/admin", { signup: mode === "open" }, "PATCH");
			toast("Saved.");
		});
</script>

<Section id="signup" label="sign up">
	<Toggle
		bind:value={() => (info.signup ? "open" : "invite"), set}
		options={[
			{ key: "invite", label: "invite only" },
			{ key: "open", label: "anyone" }
		]}
	/>
</Section>
