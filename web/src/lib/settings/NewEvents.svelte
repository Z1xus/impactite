<script lang="ts">
	import eyeOff from "pixelarticons/svg/eye-off.svg?raw";
	import plus from "pixelarticons/svg/plus.svg?raw";
	import { api, get } from "#lib/api.ts";
	import { number } from "#lib/display.svelte.ts";
	import { attempt, toast } from "#lib/toast.svelte.ts";
	import Button from "#lib/ui/Button.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import List from "#lib/ui/List.svelte";

	type Event = { name: string; count: number; revenue: boolean };

	let {
		domain,
		events = $bindable([]),
		heading = false,
		changed
	}: { domain: string; events?: Event[]; heading?: boolean; changed?: () => void } = $props();

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
		{#if heading}
			<h3 class="label text-dim">suggested goals</h3>
		{/if}
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
						<div class="mr-1 w-14">
							<Field
								size="sm"
								label="currency for {event.name}"
								placeholder="EUR"
								maxlength={3}
								bind:value={
									() => currencies[event.name] ?? "",
									(value) => (currencies[event.name] = value)
								}
							/>
						</div>
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
						class="-mr-2"
						aria-label="Ignore {event.name}"
						title="Ignore"
						onclick={() => ignore(event.name)}
					>
						<Icon svg={eyeOff} size={12} />
					</Button>
				{/if}
			{/snippet}
		</List>
		{#if events.length > 1}
			<Button variant="quiet" size="sm" class="self-end" onclick={addAll}>Add all</Button>
		{/if}
	</div>
{/if}
