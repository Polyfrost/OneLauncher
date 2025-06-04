import type { JSX } from 'react';
import React from 'react';

export interface ShowProps<T = any> {
	/**
	 * Condition to evaluate.
	 * If truthy, `children` are rendered.
	 * If T is a value (not just boolean), you might want to use it in children,
	 * but this basic version doesn't automatically pass it like SolidJS does for type narrowing.
	 */
	when: T | boolean | undefined | null;
	/**
	 * Content to render if `when` is falsy.
	 */
	fallback?: React.ReactNode;
	/**
	 * Content to render if `when` is truthy.
	 */
	children: React.ReactNode | ((item: NonNullable<T>) => React.ReactNode);
}

export function ShowComponent<T,>({ when, fallback = null, children }: ShowProps<T>): JSX.Element | null {
	if (when) {
		if (typeof children === 'function')
		// Ensure 'when' is not null or undefined before calling the function.
		// This mimics Solid's behavior where children can be a function that receives the truthy 'when' value.
			return <>{children(when as NonNullable<T>)}</>;

		return <>{children}</>;
	}
	return <>{fallback}</>;
}

export const Show = React.memo(ShowComponent) as <T>(props: ShowProps<T>) => JSX.Element | null;
