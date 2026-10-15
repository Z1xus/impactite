import { api } from "./api";
import { applyTheme, type Theme } from "./theme.svelte.ts";

// per user display choices, kept in prefs
export type Display = {
	week: "monday" | "sunday";
	clock: "24" | "12";
	numbers: "en-US" | "de-DE" | "fr-FR";
	theme: Theme;
};

export const display = $state<Display>({
	week: "monday",
	clock: "24",
	numbers: "en-US",
	theme: "dark"
});

export async function loadDisplay() {
	const saved = await api<Partial<Display> | null>("/api/app/prefs/display").catch(() => null);
	Object.assign(display, saved ?? {});
	applyTheme(display.theme);
	// follow the system while it's the choice
	matchMedia("(prefers-color-scheme: light)").addEventListener("change", () => {
		if (display.theme === "system") applyTheme("system");
	});
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

// "14:30" in the chosen clock
export function clock(time: string) {
	if (display.clock === "24") return time;
	const [hours, minutes] = time.split(":").map(Number);
	return `${hours % 12 || 12}:${String(minutes).padStart(2, "0")} ${hours < 12 ? "am" : "pm"}`;
}
