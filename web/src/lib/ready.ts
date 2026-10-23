import { session } from "./session.svelte.ts";

// holds a navigation until its data is in, a first load has no page to hold and shows its skeleton
export async function ready(...asks: Promise<unknown>[]) {
	if (!session.ready) return;
	await Promise.race([Promise.allSettled(asks), new Promise((done) => setTimeout(done, 3000))]);
}
