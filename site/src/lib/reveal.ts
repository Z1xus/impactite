// flips once when the element's top reaches the lower fifth of the screen, however tall it is
export const reveal = (shown: (value: boolean) => void) => (node: HTMLElement) => {
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
