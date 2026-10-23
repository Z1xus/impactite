export function dismiss(isOpen: () => boolean, close: () => void) {
	return (root: HTMLElement) => {
		const trigger = () => root.querySelector<HTMLElement>("[aria-expanded]");
		const down = (event: PointerEvent) => {
			if (isOpen() && !root.contains(event.target as Node)) close();
		};
		const key = (event: KeyboardEvent) => {
			if (event.key !== "Escape" || !isOpen()) return;
			event.preventDefault();
			event.stopImmediatePropagation();
			close();
			trigger()?.focus({ preventScroll: true });
		};
		const out = (event: FocusEvent) => {
			const next = event.relatedTarget as Node | null;
			if (isOpen() && next && !root.contains(next)) close();
		};
		document.addEventListener("pointerdown", down);
		document.addEventListener("keydown", key);
		root.addEventListener("focusout", out);
		return () => {
			document.removeEventListener("pointerdown", down);
			document.removeEventListener("keydown", key);
			root.removeEventListener("focusout", out);
		};
	};
}
