import { useEffect, useRef, useState } from 'react';
import type { Owner } from 'solid-js';
import { createReaction, createRoot, runWithOwner } from 'solid-js';

// copied from `useObserver` in `react-solid-state`, ported to work with new versions.
// https://github.com/solidjs/react-solid-state/issues/4
export function useObserver<T>(fn: () => T): T {
	const [_, setTick] = useState(0);
	const state = useRef({
		onUpdate: (): void => {
			state.current.firedDuringRender = true;
		},
		firstRenderFired: false,
		firedDuringRender: false,
	});

	const reaction = useRef<{ dispose: () => void; track: (fn: () => void) => void }>();

	if (!reaction.current)
		reaction.current = createRoot(dispose => ({
			dispose,
			track: createReaction(() => state.current.onUpdate()),
		}));

	useEffect(() => {
		if (state.current.firedDuringRender)
			setTick(t => t + 1);

		state.current.onUpdate = () => setTick(t => t + 1);
		state.current.firstRenderFired = true;

		return () => {
			state.current.onUpdate = () => {
				state.current.firedDuringRender = false;
			};

			if (!state.current.firstRenderFired) {
				reaction.current?.dispose();
				reaction.current = undefined;
			}
		};
	}, []);

	let rendering!: T;
	reaction.current.track(() => (rendering = fn()));
	return rendering;
}

export function useObserverWithOwner<T>(owner: Owner, fn: () => T): T {
	const [_, setTick] = useState(0);
	const state = useRef({ onUpdate: () => {} });
	const reaction = useRef<{ track: (fn: () => void) => void }>();

	if (!reaction.current)
		reaction.current = runWithOwner(owner, () => ({
			track: createReaction(() => state.current.onUpdate()),
		}))!;

	useEffect(() => state.current.onUpdate = () => setTick(t => t + 1));

	let rendering!: T;
	reaction.current.track(() => (rendering = fn()));
	return rendering;
}
