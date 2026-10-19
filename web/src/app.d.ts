declare global {
	namespace App {}
	interface Window {
		opened?: import("#lib/session.svelte.ts").Share;
		early?: Promise<import("#lib/session.svelte.ts").State>;
	}
}

export {};
