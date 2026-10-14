import { api, type Me } from "./api";

export const session = $state<{
	ready: boolean;
	setup: boolean;
	signup: boolean;
	me: Me | null;
	team: number | null;
}>({ ready: false, setup: false, signup: false, me: null, team: null });

export async function refresh() {
	const state = await api<{ setup: boolean; signup: boolean; me: Me | null }>("/api/app/state");
	session.setup = state.setup;
	session.signup = state.signup;
	adopt(state.me);
	session.ready = true;
}

export function adopt(me: Me | null) {
	session.me = me;
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
