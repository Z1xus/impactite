import { clock, number } from "#lib/display.svelte.ts";

const dates = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" });
const utc = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric", timeZone: "UTC" });

export function span(seconds: number) {
	const secs = Math.max(0, Math.round(seconds));
	const [days, hours, minutes] = [
		Math.floor(secs / 86_400),
		Math.floor((secs % 86_400) / 3600),
		Math.floor((secs % 3600) / 60)
	];
	if (days) return hours ? `${days} d ${hours} h` : `${days} d`;
	if (hours) return minutes ? `${hours} h ${minutes} min` : `${hours} h`;
	if (minutes) return `${minutes} min`;
	return `${secs} s`;
}

export const every = (seconds: number) =>
	seconds < 60 ? `${seconds} s` : seconds < 3600 ? `${seconds / 60} min` : span(seconds);

export const latency = (ms: number) =>
	ms < 1000 ? `${Math.round(ms)} ms` : `${number(ms / 1000, 1)} s`;

// never rounds a short outage up to 100
export function percent(fraction: number) {
	const value = Math.floor(fraction * 10_000) / 100;
	return `${value === 100 ? "100" : number(value, 2)}%`;
}

export function stamp(seconds: number) {
	const time = new Date(seconds * 1000);
	const [hours, minutes] = [time.getHours(), time.getMinutes()].map((n) =>
		String(n).padStart(2, "0")
	);
	return `${dates.format(time)}, ${clock(`${hours}:${minutes}`)}`;
}

export const day = (seconds: number) => utc.format(seconds * 1000);

export const hostOf = (target: string) => {
	try {
		return new URL(target.includes("://") ? target : `tcp://${target}`).hostname;
	} catch {
		return "";
	}
};

export const missed = (visits: number) =>
	`about ${number(visits)} ${visits === 1 ? "visit" : "visits"} missed`;

export const local = (text: string) => stamp(Date.parse(text.replace(" ", "T")) / 1000);
