<script lang="ts">
	import type { Snippet } from "svelte";
	import back from "pixelarticons/svg/arrow-left.svg?raw";
	import Icon from "./Icon.svelte";

	let { href, title, children }: { href: string; title: string; children: Snippet } = $props();
</script>

<!-- eslint-disable svelte/no-navigation-without-resolve -- callers pass resolved paths -->
<a
	{href}
	{title}
	class="group flex h-control items-center gap-3 transition-colors duration-100 hover:text-cyan"
>
	<Icon
		svg={back}
		size={14}
		class="text-dim transition-colors duration-100 group-hover:text-cyan"
	/>
	{@render children()}
</a>
<!-- eslint-enable svelte/no-navigation-without-resolve -->
