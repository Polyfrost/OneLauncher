import type { Owner } from 'solid-js';
import { createEffect, createReaction, createRoot, createSignal, onCleanup, runWithOwner } from 'solid-js';

export interface Observer {
	dispose: () => void;
	track: (fn: () => void) => void;
};

// copied from `useObserver` in `react-solid-state`, ported to work with new versions.
// https://github.com/solidjs/react-solid-state/issues/4
export function useObserver<T>(fn: () => T): T {
	const [tick, setTick] = createSignal(0);
	const state = {
		onUpdate: (): void => {
			state.firedDuringRender = true;
		},
		firstRenderFired: false,
		firedDuringRender: false,
	};

	let reaction: Observer | undefined;
	reaction = createRoot(dispose => ({
		dispose,
		track: createReaction(() => state.onUpdate()),
	}));

	createEffect(() => {
		if (state.firedDuringRender)
			setTick(tick() + 1);

		state.onUpdate = () => setTick(tick() + 1);
		state.firstRenderFired = true;

		onCleanup(() => {
			state.onUpdate = () => {
				state.firedDuringRender = false;
			};

			if (!state.firstRenderFired) {
				reaction?.dispose();
				reaction = undefined;
			}
		});
	});

	let rendering!: T;
	reaction.track(() => (rendering = fn()));
	return rendering;
}

export interface OwnedObserver { track: (fn: () => void) => void }

export function useObserverWithOwner<T>(owner: Owner, fn: () => T): T {
	const [tick, setTick] = createSignal(0);
	const state = { onUpdate: () => {} };

	const reaction: OwnedObserver = runWithOwner(owner, () => ({
		track: createReaction(() => state.onUpdate()),
	}))!;

	createEffect(() => {
		state.onUpdate = () => setTick(tick() + 1);
	});

	let rendering!: T;
	reaction.track(() => (rendering = fn()));
	return rendering;
}
