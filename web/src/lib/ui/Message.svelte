<script lang="ts">
	import type { Snippet } from "svelte";

	let { tone = "dim", children }: { tone?: "dim" | "bad" | "cyan"; children?: Snippet } =
		$props();

	const tones = { dim: "text-dim", bad: "text-bad", cyan: "text-cyan" };
</script>

<p
	class="label leading-4 tracking-normal normal-case empty:hidden {tones[tone]}"
	aria-live="polite"
>
	{@render children?.()}
</p>
