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

	const add = (event: Event) =>
		attempt(async () => {
			await api(`/api/app/sites/${domain}/goals`, {
				page: false,
				value: event.name,
				name: "",
				currency: event.revenue ? (currencies[event.name] || "EUR").toUpperCase() : ""
			});
			events = events.filter((item) => item.name !== event.name);
			changed?.();
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
		<p class="text-[14px] text-dim">
			Your site sent these events in the last 30 days. Add one as a goal to count its
			conversions.
			{#if events.some((event) => event.revenue)}Events with revenue need the currency they're
				paid in.{/if}
		</p>
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
						<Button variant="quiet" onclick={() => add(event)}>Add goal</Button>
						<Button variant="plain" onclick={() => ignore(event.name)}>Ignore</Button>
					</div>
				{/if}
			{/snippet}
		</List>
	</div>
{/if}
