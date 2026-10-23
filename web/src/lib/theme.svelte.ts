export type Theme = "dark" | "light" | "system";
export type Style = "pixel" | "plain";

// what the page shows now, charts redraw when it changes
export const theme = $state<{ shown: "dark" | "light"; style: Style }>({
	shown: "dark",
	style: globalThis.document?.documentElement.dataset.style === "plain" ? "plain" : "pixel"
});

export type Look = { theme: "auto" | "dark" | "light"; style: Style; accent: string };

let worn = false;

export function wear(look: Look | null) {
	worn = false;
	const root = document.documentElement;
	if (!look) {
		delete root.dataset.accent;
		const saved = localStorage.getItem("theme");
		applyTheme(
			saved === "light" || saved === "system" ? saved : "dark",
			localStorage.getItem("style") === "plain" ? "plain" : "pixel",
			false
		);
		return;
	}
	paint(look.theme === "auto" ? "system" : look.theme, look.style);
	root.dataset.accent = look.accent;
	worn = true;
}

export function applyTheme(choice: Theme, style: Style, save = true) {
	if (save) {
		localStorage.setItem("theme", choice);
		localStorage.setItem("style", style);
	}
	if (!worn) paint(choice, style);
}

function paint(choice: Theme, style: Style) {
	const light =
		choice === "light" ||
		(choice === "system" && matchMedia("(prefers-color-scheme: light)").matches);
	theme.shown = light ? "light" : "dark";
	theme.style = style;
	const root = document.documentElement;
	root.dataset.switching = "";
	root.dataset.theme = theme.shown;
	root.dataset.style = style;
	requestAnimationFrame(() => requestAnimationFrame(() => delete root.dataset.switching));
	document
		.querySelector('meta[name="theme-color"]')
		?.setAttribute("content", light ? "#f3f6f7" : "#06090b");
}
