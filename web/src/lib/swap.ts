import { tick } from "svelte";

export function swap(apply: () => void) {
	if (!document.startViewTransition || matchMedia("(prefers-reduced-motion: reduce)").matches)
		return apply();
	document.startViewTransition(async () => {
		apply();
		await tick();
	});
}
