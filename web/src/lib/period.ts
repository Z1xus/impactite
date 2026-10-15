const PERIODS = [
	"realtime",
	"day",
	"7d",
	"28d",
	"91d",
	"month",
	"year",
	"12mo",
	"all",
	"custom"
] as const;
const COMPARES = ["off", "previous", "year", "custom"] as const;
const INTERVALS = ["minute", "hour", "day", "week", "month"] as const;
export type Period = (typeof PERIODS)[number];
export type Compare = (typeof COMPARES)[number];
export type Interval = (typeof INTERVALS)[number];
export type Range = [string, string] | "all" | "30m";

export const PRESETS: {
	key: string;
	label: string;
	period: Period;
	back?: number;
	hotkey: string;
}[] = [
	{ key: "day", label: "today", period: "day", hotkey: "d" },
	{ key: "yesterday", label: "yesterday", period: "day", back: 1, hotkey: "e" },
	{ key: "realtime", label: "realtime", period: "realtime", hotkey: "r" },
	{ key: "7d", label: "last 7 days", period: "7d", hotkey: "w" },
	{ key: "28d", label: "last 28 days", period: "28d", hotkey: "f" },
	{ key: "91d", label: "last 91 days", period: "91d", hotkey: "n" },
	{ key: "month", label: "month to date", period: "month", hotkey: "m" },
	{ key: "last-month", label: "last month", period: "month", back: 1, hotkey: "" },
	{ key: "year", label: "year to date", period: "year", hotkey: "y" },
	{ key: "12mo", label: "last 12 months", period: "12mo", hotkey: "l" },
	{ key: "all", label: "all time", period: "all", hotkey: "a" }
];

export type View = {
	period: Period;
	date: string;
	from: string;
	to: string;
	compare: Compare;
	compareFrom: string;
	compareTo: string;
	weekday: boolean;
	interval: Interval | null;
	today: string;
};

export function today(timezone: string) {
	return new Intl.DateTimeFormat("en-CA", { timeZone: timezone }).format(new Date());
}

export function shift(date: string, days: number) {
	const d = new Date(`${date}T00:00:00Z`);
	d.setUTCDate(d.getUTCDate() + days);
	return d.toISOString().slice(0, 10);
}

export function shiftMonths(date: string, months: number) {
	const d = new Date(`${date.slice(0, 7)}-01T00:00:00Z`);
	d.setUTCMonth(d.getUTCMonth() + months);
	return d.toISOString().slice(0, 10);
}

const lastOfMonth = (date: string) => shift(shiftMonths(date, 1), -1);
const min = (a: string, b: string) => (a < b ? a : b);
export const length = (from: string, to: string) =>
	Math.round((Date.parse(to) - Date.parse(from)) / 86_400_000) + 1;

export function parse(params: { get(name: string): string | null }, timezone: string): View {
	const now = today(timezone);
	// links can carry anything, unknown values fall back
	const known = <T extends string>(list: readonly T[], value: string | null) =>
		list.find((item) => item === value);
	return {
		period: known(PERIODS, params.get("period")) ?? "28d",
		date: params.get("date") || now,
		from: params.get("from") || now,
		to: params.get("to") || now,
		compare: known(COMPARES, params.get("compare")) ?? "off",
		compareFrom: params.get("compare_from") || now,
		compareTo: params.get("compare_to") || now,
		weekday: params.get("match") === "weekday",
		interval: known(INTERVALS, params.get("interval")) ?? null,
		today: now
	};
}

export function current(view: View): Range {
	const { period, date, today } = view;
	switch (period) {
		case "day":
			return [date, date];
		case "7d":
		case "28d":
		case "91d":
			return [shift(today, 1 - parseInt(period)), today];
		case "month":
			return [`${date.slice(0, 7)}-01`, min(lastOfMonth(date), today)];
		case "year":
			return [`${date.slice(0, 4)}-01-01`, min(`${date.slice(0, 4)}-12-31`, today)];
		case "12mo":
			return [shiftMonths(today, -11), today];
		case "custom":
			return [view.from, view.to];
		case "all":
			return "all";
		case "realtime":
			return "30m";
	}
}

export function previous(view: View, range: Range): Range | null {
	if (range === "all" || range === "30m") return null;
	const [from, to] = range;
	const days = length(from, to);
	switch (view.compare) {
		case "custom":
			return [view.compareFrom, view.compareTo];
		case "year": {
			const back = view.weekday ? 364 : 0;
			return back
				? [shift(from, -back), shift(to, -back)]
				: [shiftYears(from, -1), shiftYears(to, -1)];
		}
		default: {
			const back = view.weekday ? Math.ceil(days / 7) * 7 : days;
			return [shift(from, -back), shift(to, -back)];
		}
	}
}

function shiftYears(date: string, years: number) {
	return `${Number(date.slice(0, 4)) + years}${date.slice(4)}`;
}

export function intervals(range: Range): Interval[] {
	if (range === "30m") return ["minute"];
	if (range === "all") return ["week", "month"];
	const days = length(...range);
	if (days === 1) return ["hour"];
	if (days <= 31) return ["day", "week"];
	if (days <= 366) return ["day", "week", "month"];
	return ["week", "month"];
}

export function interval(view: View, range: Range): Interval {
	const allowed = intervals(range);
	if (view.interval && allowed.includes(view.interval)) return view.interval;
	if (range === "all" || view.period === "year" || view.period === "12mo") return "month";
	if (range === "30m") return "minute";
	return allowed[0];
}

const format = (date: string, options: Intl.DateTimeFormatOptions) =>
	new Date(`${date}T00:00:00Z`).toLocaleDateString("en-US", { timeZone: "UTC", ...options });

export function label(view: View): string {
	const { period, date, today } = view;
	if (period === "day")
		return date === today
			? "today"
			: date === shift(today, -1)
				? "yesterday"
				: format(date, { month: "short", day: "numeric", year: "numeric" });
	if (period === "month")
		return date.slice(0, 7) === today.slice(0, 7)
			? "month to date"
			: format(date, { month: "long", year: "numeric" });
	if (period === "year")
		return date.slice(0, 4) === today.slice(0, 4) ? "year to date" : date.slice(0, 4);
	if (period === "custom")
		return `${format(view.from, { month: "short", day: "numeric" })} - ${format(view.to, {
			month: "short",
			day: "numeric",
			year: "numeric"
		})}`;
	return PRESETS.find((preset) => preset.period === period)?.label ?? period;
}

export function preset(key: string, view: View): Record<string, string> {
	const found = PRESETS.find((item) => item.key === key);
	if (!found) return {};
	const date =
		found.period === "day"
			? shift(view.today, -(found.back ?? 0))
			: shiftMonths(view.today, -(found.back ?? 0));
	return found.back ? { period: found.period, date } : { period: found.period };
}

export function step(view: View, direction: -1 | 1): string | null {
	const next =
		view.period === "day"
			? shift(view.date, direction)
			: view.period === "month"
				? shiftMonths(view.date, direction)
				: view.period === "year"
					? `${Number(view.date.slice(0, 4)) + direction}${view.date.slice(4)}`
					: null;
	if (!next) return null;
	const start = view.period === "day" ? next : `${next.slice(0, view.period === "year" ? 4 : 7)}`;
	return start > view.today.slice(0, start.length) ? null : next;
}
