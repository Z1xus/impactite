import { api } from "#lib/api.ts";
import { session } from "#lib/session.svelte.ts";

type Saved = { order: string[]; hidden: string[]; table: boolean };

const list = (value: unknown) =>
	Array.isArray(value) ? value.filter((item) => typeof item === "string") : [];

export function createLayout(domain: () => string, readonly: () => boolean) {
	const key = () => `layout:${domain()}`;
	const saved = $derived.by<Saved>(() => {
		const value = session.prefs?.[key()] as Partial<Saved> | null | undefined;
		return {
			order: list(value?.order),
			hidden: list(value?.hidden),
			table: value?.table === true
		};
	});
	let arranging = $state(false);

	function save(next: Saved) {
		if (readonly() || !session.prefs) return;
		session.prefs[key()] = next;
		api(`/api/app/prefs/${key()}`, next, "PUT");
	}

	return {
		get order() {
			return saved.order;
		},
		get hidden() {
			return saved.hidden;
		},
		get table() {
			return saved.table;
		},
		get ready() {
			return readonly() || session.prefs !== null;
		},
		get arranging() {
			return arranging;
		},
		set arranging(value: boolean) {
			arranging = value;
		},
		isHidden: (id: string) => saved.hidden.includes(id),
		setOrder: (order: string[]) => save({ ...saved, order }),
		setTable: (table: boolean) => save({ ...saved, table }),
		hide: (id: string) => save({ ...saved, hidden: [...saved.hidden, id] }),
		show: (id: string) => save({ ...saved, hidden: saved.hidden.filter((item) => item !== id) })
	};
}
