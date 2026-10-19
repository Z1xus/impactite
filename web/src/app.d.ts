declare global {
	namespace App {}
	interface Window {
		early?: Promise<import("#lib/session.svelte.ts").State>;
	}
}

export {};
