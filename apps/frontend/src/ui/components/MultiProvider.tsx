/**
 * Taken from https://github.com/solidjs-community/solid-primitives/blob/main/packages/context/src/index.ts
 * MIT LICENSE
 */
/* eslint-disable solid/reactivity -- . */
import { type Context, type ContextProviderComponent, type FlowComponent, type JSX, createComponent } from 'solid-js';

export function MultiProvider<T extends readonly [unknown?, ...unknown[]]>(props: {
	values: {
		[K in keyof T]:
			| readonly [
				Context<T[K]> | ContextProviderComponent<T[K]>,
				[T[K]][T extends unknown ? 0 : never],
			]
			| FlowComponent;
	};
	children: JSX.Element;
}): JSX.Element {
	const { values } = props;
	const fn = (i: number) => {
		let item: any = values[i];

		if (!item)
			return props.children;

		const ctxProps: { value?: any; children: JSX.Element } = {
			get children() {
				return fn(i + 1);
			},
		};
		if (Array.isArray(item)) {
			ctxProps.value = item[1];
			item = item[0];
			if (typeof item !== 'function')
				item = item.Provider;
		}

		return createComponent(item, ctxProps);
	};
	return fn(0);
}
