declare global {
	namespace App {}
	interface Window {
		opened?: unknown;
		early?: Promise<import("#lib/session.svelte.ts").State>;
	}
}

export {};
