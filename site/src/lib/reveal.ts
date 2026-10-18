import { prefersReducedMotion } from "svelte/motion";

// the markup renders the final state. unless motion is reduced, this resets to the start, then flips once when the element's top reaches the lower fifth of the screen
export const reveal = (shown: (value: boolean) => void) => (node: HTMLElement) => {
	if (prefersReducedMotion.current) return;
	shown(false);
	const observer = new IntersectionObserver(
		([entry]) => {
			if (!entry.isIntersecting) return;
			shown(true);
			observer.disconnect();
		},
		{ rootMargin: "0px 0px -20% 0px" }
	);
	observer.observe(node);
	return () => observer.disconnect();
};
