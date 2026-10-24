import type { current, interval, intervals, previous } from "#lib/period.ts";
import type { Filter } from "#lib/stats.ts";

export type Dates = {
	current: ReturnType<typeof current>;
	previous: ReturnType<typeof previous>;
	overlay: boolean;
	interval: ReturnType<typeof interval>;
	grain: string;
	choices: ReturnType<typeof intervals>;
};

export type Funnel = { id: number; name: string };
export type Note = { id: number; day: string; text: string };

// what every part of a dashboard needs to ask the same question
export type Context = {
	domain: string;
	combined: boolean;
	dates: Dates;
	filters: Filter[];
	imports: boolean;
	version: number;
	converting: boolean;
	titles: Record<string, string>;
	goals: string[];
	propKeys: string[];
	funnels: Funnel[];
	vitals: boolean;
	add: (dim: string, value: string) => void;
	expand: (dim: string, title: string) => void;
};

type Tab = { key: string; label: string };
export type PanelSpec = { id: string; tabs: Tab[] };
