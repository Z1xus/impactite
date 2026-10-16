<script lang="ts">
	import { sentence } from "./text";
	import { asking } from "#lib/ask.svelte.ts";
	import Button from "./Button.svelte";
	import Dialog from "./Dialog.svelte";
</script>

<Dialog
	title={asking.question?.title ?? ""}
	bind:open={() => asking.question !== null, (open) => !open && asking.answer(false)}
>
	<div class="flex flex-col gap-5">
		{#if asking.question?.body}
			<p class="text-[14px] text-dim">{asking.question.body}</p>
		{/if}
		<div class="flex justify-end gap-2">
			<Button variant="quiet" onclick={() => asking.answer(false)}>Cancel</Button>
			<Button variant="danger" onclick={() => asking.answer(true)}>
				{sentence(asking.question?.action ?? "")}
			</Button>
		</div>
	</div>
</Dialog>
