import { api, type Me } from "./api";
import { loadDisplay } from "./display.svelte.ts";

export const session = $state<{
	ready: boolean;
	setup: boolean;
	signup: boolean;
	me: Me | null;
	team: number | null;
	alias: string | null;
	sso: string | null;
}>({ ready: false, setup: false, signup: false, me: null, team: null, alias: null, sso: null });

export async function refresh() {
	const state = await api<{
		setup: boolean;
		signup: boolean;
		me: Me | null;
		alias: string | null;
		sso: string | null;
	}>("/api/app/state");
	session.setup = state.setup;
	session.signup = state.signup;
	session.alias = state.alias;
	session.sso = state.sso;
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
