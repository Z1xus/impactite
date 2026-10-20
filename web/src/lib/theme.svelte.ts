export type Theme = "dark" | "light" | "system";
export type Style = "pixel" | "plain";

// what the page shows now, charts redraw when it changes
export const theme = $state<{ shown: "dark" | "light"; style: Style }>({
	shown: "dark",
	style: globalThis.document?.documentElement.dataset.style === "plain" ? "plain" : "pixel"
});

export function applyTheme(choice: Theme, style: Style) {
	localStorage.setItem("theme", choice);
	localStorage.setItem("style", style);
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
