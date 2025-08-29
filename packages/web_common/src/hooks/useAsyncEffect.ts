import type React from 'react';
import { useEffect } from 'react';

type UnlistenFn = (() => any) | undefined;

export function useAsyncEffect(cb: () => Promise<UnlistenFn>, deps?: React.DependencyList) {
	useEffect(() => {
		let unlisten: UnlistenFn;

		cb().then((fn) => {
			unlisten = fn;
		});

		return () => {
			unlisten?.();
		};
	// eslint-disable-next-line react-hooks/exhaustive-deps -- the function should not depend on the cb but the deps list
	}, deps);
}
