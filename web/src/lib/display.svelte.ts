import { api } from "./api";
import { applyTheme, type Theme } from "./theme.svelte.ts";

export type Display = {
	week: "monday" | "sunday";
	clock: "24" | "12";
	numbers: "en-US" | "de-DE" | "fr-FR";
	theme: Theme;
	all: "show" | "hide";
};

export const display = $state<Display>({
	week: "monday",
	clock: "24",
	numbers: "en-US",
	theme: "dark",
	all: "show"
});

const choices: { [key in keyof Display]: Display[key][] } = {
	week: ["monday", "sunday"],
	clock: ["24", "12"],
	numbers: ["en-US", "de-DE", "fr-FR"],
	theme: ["dark", "light", "system"],
	all: ["show", "hide"]
};

// follow the system while it's the choice
globalThis.matchMedia?.("(prefers-color-scheme: light)").addEventListener("change", () => {
	if (display.theme === "system") applyTheme("system");
});

export function loadDisplay(value: unknown) {
	const saved = value as Partial<Display> | null | undefined;
	// a stored value from an older version or a bad write falls back to the default
	for (const key of Object.keys(choices) as (keyof Display)[])
		if ((choices[key] as unknown[]).includes(saved?.[key]))
			Object.assign(display, { [key]: saved?.[key] });
	applyTheme(display.theme);
}

export function saveDisplay(next: Partial<Display>) {
	Object.assign(display, next);
	applyTheme(display.theme);
	return api("/api/app/prefs/display", { ...display }, "PUT");
}

export const number = (value: number, digits = 0) =>
	value.toLocaleString(display.numbers, {
		minimumFractionDigits: digits,
		maximumFractionDigits: digits
	});

const days = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric", year: "numeric" });
export const date = (seconds: number) => days.format(seconds * 1000);

// "14:30" in the chosen clock
export function clock(time: string) {
	if (display.clock === "24") return time;
	const [hours, minutes] = time.split(":").map(Number);
	return `${hours % 12 || 12}:${String(minutes).padStart(2, "0")} ${hours < 12 ? "am" : "pm"}`;
}
