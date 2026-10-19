import { api, type Me, type Team } from "./api";
import { loadDisplay } from "./display.svelte.ts";

export type Share = { domain: string; timezone: string; name: string; token: string };

export type State = {
	setup: boolean;
	signup: boolean;
	me: Me | null;
	alias: string | null;
	sso: string | null;
	update: string | null;
};

export const session = $state<{
	ready: boolean;
	setup: boolean;
	signup: boolean;
	me: Me | null;
	team: number | null;
	alias: string | null;
	sso: string | null;
	update: string | null;
}>({
	ready: false,
	setup: false,
	signup: false,
	me: null,
	team: null,
	alias: null,
	sso: null,
	update: null
});

export async function refresh() {
	const early = window.early;
	window.early = undefined;
	const state = (await early?.catch(() => null)) ?? (await api<State>("/api/app/state"));
	session.setup = state.setup;
	session.signup = state.signup;
	session.alias = state.alias;
	session.sso = state.sso;
	session.update = state.update;
	adopt(state.me);
	session.ready = true;
}

export function adopt(me: Me | null) {
	session.me = me;
	if (me) loadDisplay();
	const saved = Number(localStorage.getItem("team"));
	const teams = me?.teams ?? [];
	session.team = teams.find((team) => team.id === saved)?.id ?? teams[0]?.id ?? null;
}

export function pickTeam(id: number) {
	session.team = id;
	localStorage.setItem("team", String(id));
}

export function currentTeam() {
	return session.me?.teams.find((team) => team.id === session.team) ?? null;
}

export function findSite(domain: string) {
	return session.me?.teams.flatMap((team) => team.sites).find((site) => site.domain === domain);
}

// the server reads a team in the timezone of its oldest site
export function teamTimezone(team: Team) {
	return [...team.sites].sort((a, b) => a.id - b.id)[0]?.timezone ?? "UTC";
}
