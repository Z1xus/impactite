import { tick } from "svelte";

// replaces content in one frame with a short crossfade
export function swap(apply: () => void) {
	if (!document.startViewTransition || matchMedia("(prefers-reduced-motion: reduce)").matches)
		return apply();
	document.startViewTransition(async () => {
		apply();
		await tick();
	});
}
