export type Question = { title: string; body?: string; action: string };

export const asking = $state<{ question: Question | null; answer: (yes: boolean) => void }>({
	question: null,
	answer: () => {}
});

// asks before something that can't be undone, true when the person goes ahead
export function ask(question: Question): Promise<boolean> {
	asking.answer(false);
	return new Promise((resolve) => {
		asking.question = question;
		asking.answer = (yes) => {
			asking.question = null;
			asking.answer = () => {};
			resolve(yes);
		};
	});
}
