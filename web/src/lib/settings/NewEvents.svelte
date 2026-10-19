<script lang="ts">
	import eyeOff from "pixelarticons/svg/eye-off.svg?raw";
	import plus from "pixelarticons/svg/plus.svg?raw";
	import { api, get } from "#lib/api.ts";
	import { number } from "#lib/display.svelte.ts";
	import { attempt, toast } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import List from "#lib/ui/List.svelte";

	type Event = { name: string; count: number; revenue: boolean };

	let {
		domain,
		events = $bindable([]),
		changed
	}: { domain: string; events?: Event[]; changed?: () => void } = $props();

	let currencies = $state<Record<string, string>>({});

	export function load() {
		get<{ events: Event[] }>(`/api/app/sites/${domain}/goals/suggestions`)
			.then((r) => (events = r.events))
			.catch(() => (events = []));
	}

	$effect(() => {
		void domain;
		load();
	});

	const create = async (event: Event) => {
		await api(`/api/app/sites/${domain}/goals`, {
			page: false,
			value: event.name,
			name: "",
			currency: event.revenue ? (currencies[event.name] || "EUR").toUpperCase() : ""
		});
		events = events.filter((item) => item.name !== event.name);
	};

	const add = (event: Event) =>
		attempt(async () => {
			await create(event);
			changed?.();
			toast("Added.");
		});

	const addAll = () =>
		attempt(async () => {
			try {
				for (const event of [...events]) await create(event);
				toast("Added.");
			} finally {
				changed?.();
			}
		});

	const ignore = (name: string) =>
		attempt(async () => {
			events = (
				await api<{ events: Event[] }>(`/api/app/sites/${domain}/goals/suggestions`, {
					name
				})
			).events;
			changed?.();
		});
</script>

{#if events.length}
	<div
		class="-mx-5 flex flex-col gap-4 px-5 not-first:border-t not-first:border-line not-first:pt-4"
	>
		<div class="flex min-h-control items-center justify-between gap-4">
			<h3 class="label text-dim">suggested goals</h3>
			<Button variant="quiet" onclick={addAll}>Add all</Button>
		</div>
		<List
			items={events.map((event) => ({
				key: event.name,
				title: event.name,
				meta: [
					`${number(event.count)} ${event.count === 1 ? "time" : "times"}`,
					...(event.revenue ? ["with revenue"] : [])
				]
			}))}
		>
			{#snippet control(key)}
				{@const event = events.find((event) => event.name === key)}
				{#if event}
					{#if event.revenue}
						<input
							bind:value={currencies[event.name]}
							placeholder="EUR"
							maxlength={3}
							aria-label="currency for {event.name}"
							class="mr-1 h-control w-16 rounded-px border border-control bg-panel px-3 text-fg uppercase outline-none placeholder:text-faint hover:border-dim focus:border-cyan"
						/>
					{/if}
					<Button
						variant="tool"
						aria-label="Add {event.name}"
						title="Add goal"
						onclick={() => add(event)}
					>
						<Icon svg={plus} size={12} />
					</Button>
					<Button
						variant="tool"
						class="-mr-3"
						aria-label="Ignore {event.name}"
						title="Ignore"
						onclick={() => ignore(event.name)}
					>
						<Icon svg={eyeOff} size={12} />
					</Button>
				{/if}
			{/snippet}
		</List>
	</div>
{/if}
