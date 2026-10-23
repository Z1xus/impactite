<script lang="ts">
	import { untrack } from "svelte";
	import arrowDown from "pixelarticons/svg/arrow-down.svg?raw";
	import arrowUp from "pixelarticons/svg/arrow-up.svg?raw";
	import Button from "#lib/ui/Button.svelte";
	import Checks from "#lib/ui/Checks.svelte";
	import Copy from "#lib/ui/Copy.svelte";
	import Dialog from "#lib/ui/Dialog.svelte";
	import Field from "#lib/ui/Field.svelte";
	import Form from "#lib/ui/Form.svelte";
	import Icon from "#lib/ui/Icon.svelte";
	import Pick from "#lib/ui/Pick.svelte";
	import Toggle from "#lib/ui/Toggle.svelte";
	import StatusPreview from "./StatusPreview.svelte";
	import type { Look, Monitor, Page } from "./types.ts";

	let {
		open = $bindable(false),
		page,
		monitors,
		save
	}: {
		open: boolean;
		page?: Page;
		monitors: Monitor[];
		save: (body: Record<string, unknown>) => Promise<unknown>;
	} = $props();

	let name = $state("");
	let slug = $state("");
	let password = $state("");
	let chosen = $state<string[]>([]);
	let clear = $state<string[]>([]);
	let edited = $state(false);
	let look = $state<Look>({
		theme: "auto",
		style: "pixel",
		accent: "cyan",
		brand: true,
		home: ""
	});

	const ACCENTS: { key: Look["accent"]; label: string; color: string }[] = [
		{ key: "cyan", label: "Cyan", color: "#38e1f2" },
		{ key: "green", label: "Green", color: "#4ade80" },
		{ key: "blue", label: "Blue", color: "#6aa8ff" },
		{ key: "violet", label: "Violet", color: "#a991ff" },
		{ key: "mono", label: "Mono", color: "#8798a0" }
	];

	const options = $derived(monitors.map((item) => ({ key: String(item.id), label: item.name })));
	const named = $derived(
		Object.fromEntries(monitors.map((item) => [String(item.id), item.name]))
	);
	const slugify = (text: string) =>
		text
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, "-")
			.replace(/^-|-$/g, "")
			.slice(0, 40)
			.replace(/-$/, "");

	$effect(() => {
		if (!open) return;
		untrack(() => {
			name = page?.name ?? "";
			slug = page?.slug ?? "";
			edited = !!page;
			password = "";
			clear = [];
			chosen = page?.monitors.map(String) ?? [];
			look = page?.look
				? { ...page.look }
				: { theme: "auto", style: "pixel", accent: "cyan", brand: true, home: "" };
		});
	});

	function rename(value: string) {
		name = value;
		if (!edited) slug = slugify(value);
	}

	function move(at: number, step: number) {
		const next = [...chosen];
		[next[at], next[at + step]] = [next[at + step], next[at]];
		chosen = next;
	}

	const badge = (id: string) =>
		`![${named[id] ?? "status"}](${location.origin}/status/${page?.slug}/${id}.svg)`;

	async function submit() {
		await save({
			name,
			slug,
			monitors: chosen.map(Number),
			look,
			...(password || clear.length ? { password } : {})
		});
		open = false;
	}
</script>

{#snippet preview()}
	<StatusPreview
		{look}
		title={name.trim()}
		monitors={chosen.flatMap((id) => monitors.filter((item) => String(item.id) === id))}
	/>
{/snippet}

<Dialog title={page ? "edit status page" : "add status page"} bind:open wide>
	<div class="grid gap-6 md:grid-cols-[minmax(0,1fr)_17rem]">
		<div class="sticky top-0 order-last self-start max-md:hidden">
			{@render preview()}
		</div>
		<Form action={page ? "Save" : "Add page"} done="Saved." {submit}>
			<Field
				label="title"
				placeholder="Acme status"
				maxlength={80}
				bind:value={() => name, rename}
			/>
			<Field
				label="link"
				prefix="{location.host}/status/"
				placeholder="acme"
				maxlength={40}
				autocapitalize="off"
				spellcheck={false}
				bind:value={
					() => slug,
					(value: string) => {
						slug = value;
						edited = true;
					}
				}
				required
			/>
			<div class="flex flex-col gap-2">
				<span class="label leading-[14px]">monitors</span>
				<Checks bind:value={chosen} {options} data />
			</div>
			{#if chosen.length > 1 || (page && chosen.length)}
				<span class="label leading-[14px]">order</span>
				<ul class="-mt-2 flex flex-col divide-y divide-line">
					{#each chosen as id, at (id)}
						<li class="flex items-center gap-1 py-1">
							<span class="min-w-0 flex-1 truncate text-body text-fg"
								>{named[id] ?? id}</span
							>
							{#if chosen.length > 1}
								<Button
									variant="tool"
									aria-label="Move up"
									title="Move up"
									disabled={at === 0}
									onclick={() => move(at, -1)}
								>
									<Icon svg={arrowUp} size={12} />
								</Button>
								<Button
									variant="tool"
									aria-label="Move down"
									title="Move down"
									disabled={at === chosen.length - 1}
									onclick={() => move(at, 1)}
								>
									<Icon svg={arrowDown} size={12} />
								</Button>
							{/if}
							{#if page}
								<Copy text={badge(id)} label="Copy badge" />
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
			<Field
				label="password"
				type="password"
				placeholder={page?.locked ? "Unchanged" : "None"}
				bind:value={password}
			/>
			{#if page?.locked}
				<Checks bind:value={clear} options={[{ key: "clear", label: "Remove password" }]} />
			{/if}
			<div class="md:hidden">{@render preview()}</div>
			<div class="grid gap-4 sm:grid-cols-2">
				<Toggle
					label="theme"
					bind:value={look.theme}
					options={[
						{ key: "auto", label: "Auto" },
						{ key: "dark", label: "Dark" },
						{ key: "light", label: "Light" }
					]}
				/>
				<Toggle
					label="style"
					bind:value={look.style}
					options={[
						{ key: "pixel", label: "Pixel" },
						{ key: "plain", label: "Plain" }
					]}
				/>
			</div>
			<div class="flex flex-col gap-2">
				<span class="label leading-[14px]">accent</span>
				<div class="flex flex-wrap gap-1.5" role="group" aria-label="accent">
					{#each ACCENTS as item (item.key)}
						<Pick
							swatch={item.color}
							label={item.label}
							on={look.accent === item.key}
							onclick={() => (look.accent = item.key)}
						/>
					{/each}
				</div>
			</div>
			<Field
				label="header link"
				type="url"
				placeholder="https://example.com"
				bind:value={look.home}
			/>
			<Checks
				bind:value={
					() => (look.brand ? ["brand"] : []),
					(value: string[]) => (look.brand = value.includes("brand"))
				}
				options={[{ key: "brand", label: "Show impactite" }]}
			/>
		</Form>
	</div>
</Dialog>
