export type Theme = "dark" | "light" | "system";

// what the page shows now, charts redraw when it changes
export const theme = $state<{ shown: "dark" | "light" }>({ shown: "dark" });

export function applyTheme(choice: Theme) {
	localStorage.setItem("theme", choice);
	const light =
		choice === "light" ||
		(choice === "system" && matchMedia("(prefers-color-scheme: light)").matches);
	theme.shown = light ? "light" : "dark";
	const root = document.documentElement;
	root.dataset.switching = "";
	root.dataset.theme = theme.shown;
	requestAnimationFrame(() => requestAnimationFrame(() => delete root.dataset.switching));
	document
		.querySelector('meta[name="theme-color"]')
		?.setAttribute("content", light ? "#f3f6f7" : "#06090b");
}
