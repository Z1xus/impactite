<script lang="ts">
	import { api } from "#lib/api.ts";
	import { number } from "#lib/display.svelte.ts";
	import { attempt } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import List from "#lib/ui/List.svelte";

	type Event = { name: string; count: number; revenue: boolean };

	let {
		domain,
		events = $bindable([]),
		changed
	}: { domain: string; events?: Event[]; changed?: () => void } = $props();

	let currencies = $state<Record<string, string>>({});

	export function load() {
		api<{ events: Event[] }>(`/api/app/sites/${domain}/goals/suggestions`)
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
		});

	const addAll = () =>
		attempt(async () => {
			try {
				for (const event of [...events]) await create(event);
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
	<div class="flex flex-col gap-4">
		<p class="text-[14px] text-dim">Events from the last 30 days that have no goal.</p>
		<div class="max-h-[50vh] [scrollbar-gutter:stable] overflow-y-auto">
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
						<div class="flex items-center gap-2">
							{#if event.revenue}
								<input
									bind:value={currencies[event.name]}
									placeholder="EUR"
									maxlength={3}
									aria-label="currency for {event.name}"
									class="h-9 w-16 rounded-px border border-edge bg-panel px-3 text-fg uppercase outline-none placeholder:text-faint hover:border-dim focus:border-cyan"
								/>
							{/if}
							<Button variant="quiet" onclick={() => add(event)}>Add</Button>
							<Button variant="plain" onclick={() => ignore(event.name)}
								>Ignore</Button
							>
						</div>
					{/if}
				{/snippet}
			</List>
		</div>
		<div class="flex justify-end">
			<Button variant="quiet" onclick={addAll}>Add all</Button>
		</div>
	</div>
{/if}
