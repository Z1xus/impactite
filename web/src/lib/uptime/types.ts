export type Health = "up" | "slow" | "down" | "paused" | "maintenance" | "pending";
export type Kind = "http" | "tcp" | "heartbeat";
export type Cell = [number, "up" | "slow" | "down", number] | null;

export type Check = {
	id: number;
	state: Health;
	since: number;
	latency: number | null;
	at: number;
	cert_expires: number | null;
};

export type Monitor = Check & {
	name: string;
	kind: Kind;
	target: string;
	site: string | null;
	interval: number;
	paused: boolean;
	muted: boolean;
	uptime_24h: number | null;
	uptime_30d: number | null;
	p50_24h: number | null;
	strip: Cell[];
};

export type Config = {
	method: string;
	status: string;
	contains: string;
	lacks: string;
	headers: Record<string, string>;
	timeout: number;
	slow: number;
	grace: number;
};

export type Note = { id: number; at: number; text: string; public: boolean };

export type Incident = {
	id: number;
	started_at: number;
	ended_at: number | null;
	cause: string;
	detail: string;
	duration: number;
	visits_missed: number | null;
	notes: Note[];
};

export type Range = "24h" | "7d" | "30d" | "90d";

export type Detail = {
	monitor: Monitor & { config: Config; created_at: number };
	role: string;
	range: Range;
	kpis: {
		uptime: number | null;
		downtime: number | null;
		incidents: number;
		p50: number | null;
		p95: number | null;
		cert_expires: number | null;
	};
	series: { t: number; ms: number | null; state: Health }[];
	incidents: Incident[];
};

export type Window = {
	id: number;
	starts_at: number;
	ends_at: number;
	note: string;
	monitors: number[];
};

export const INTERVALS = [30, 60, 120, 300, 600, 900, 1800, 3600];
export const BEATS = [60, 300, 900, 3600, 21_600, 86_400, 604_800];

export const writes = (role: string | undefined) =>
	["owner", "admin", "editor"].includes(role ?? "");

export type Page = {
	id: number;
	slug: string;
	name: string;
	locked: boolean;
	monitors: number[];
	look: Look;
};

export type Listed = Pick<Monitor, "id" | "name">;

export type Look = {
	theme: "auto" | "dark" | "light";
	style: "pixel" | "plain";
	accent: "cyan" | "green" | "blue" | "violet" | "mono";
	brand: boolean;
	home: string;
};

export type Status = {
	name: string;
	look: Look;
	token?: string;
	monitors: (Pick<Monitor, "id" | "name" | "state" | "since" | "strip"> & {
		uptime: number | null;
	})[];
	incidents: {
		id: number;
		monitor: number;
		started_at: number;
		ended_at: number | null;
		cause: string;
		notes: { at: number; text: string }[];
	}[];
	maintenance: Window[];
};

export type Imported = {
	monitors: number;
	existing: number;
	checks: number;
	incidents: number;
	alerts: number;
	maintenance: number;
	pages: number;
	skipped: { name: string; reason: string }[];
	noted: { name: string; reason: string }[];
};

export type Alert = {
	id: number;
	kind: import("#lib/channels.ts").Channel;
	label: string;
	sent_at: number | null;
	failure: string;
};
