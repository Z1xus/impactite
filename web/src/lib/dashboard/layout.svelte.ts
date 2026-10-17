import { api } from "#lib/api.ts";

type Saved = { order: string[]; hidden: string[]; table: boolean };

const list = (value: unknown) =>
	Array.isArray(value) ? value.filter((item) => typeof item === "string") : [];

export function createLayout(domain: () => string, readonly: () => boolean) {
	let saved = $state<Saved>({ order: [], hidden: [], table: false });
	let arranging = $state(false);

	$effect(() => {
		if (readonly()) return;
		api<Partial<Saved> | null>(`/api/app/prefs/layout:${domain()}`).then((value) => {
			saved = {
				order: list(value?.order),
				hidden: list(value?.hidden),
				table: value?.table === true
			};
		});
	});

	function save(next: Saved) {
		saved = next;
		if (!readonly()) api(`/api/app/prefs/layout:${domain()}`, next, "PUT");
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
