import { afterNavigate, beforeNavigate } from "$app/navigation";

const STORE = "scroll";
const slot = (): number | undefined => history.state?.["sveltekit:metadata"]?.historyIndex;
const saved = (): Record<number, number> => JSON.parse(sessionStorage.getItem(STORE) ?? "{}");

// remembers where each history entry was scrolled and holds that spot until late content has filled the page
export function keepScroll() {
	let frozen = false;
	let release = () => {};

	function hold(target: number) {
		release();
		const root = document.documentElement;
		const started = performance.now();
		let frame = 0;
		release = () => {
			cancelAnimationFrame(frame);
			removeEventListener("wheel", release);
			removeEventListener("touchstart", release);
			removeEventListener("keydown", release);
			root.style.minHeight = "";
			frozen = false;
			release = () => {};
		};
		frozen = true;
		root.style.minHeight = `${target + innerHeight}px`;
		scrollTo(0, target);
		addEventListener("wheel", release, { passive: true });
		addEventListener("touchstart", release, { passive: true });
		addEventListener("keydown", release);
		// content that never grows tall enough stops holding after a while
		const wait = () =>
			document.body.scrollHeight >= target + innerHeight || performance.now() - started > 4000
				? release()
				: (frame = requestAnimationFrame(wait));
		wait();
	}

	function save() {
		const at = slot();
		if (at) sessionStorage.setItem(STORE, JSON.stringify({ ...saved(), [at]: scrollY }));
	}

	const start = saved()[slot() ?? 0];
	if (start) hold(start);

	addEventListener(
		"scroll",
		() => {
			if (!frozen) save();
		},
		{ passive: true }
	);
	beforeNavigate((navigation) => {
		// by then history already points at the entry being returned to
		if (!frozen && navigation.type !== "popstate") save();
		release();
		frozen = true;
	});
	afterNavigate((navigation) => {
		if (navigation.type === "popstate") hold(saved()[slot() ?? 0] ?? 0);
		// the new page's own scroll reset must not count
		else requestAnimationFrame(() => requestAnimationFrame(() => (frozen = false)));
	});
}
