declare global {
	// eslint-disable-next-line vars-on-top, no-var -- global patched variable
	var isDevelopment: boolean;
	// eslint-disable-next-line vars-on-top, no-var -- global patched variable
	var onHotReload: (cb: () => void) => void | undefined;
}

if (globalThis.isDevelopment === undefined || globalThis.localStorage === undefined)
	throw new Error('please ensure you have patched `globalThis` before importing the client');

declare global {
	// global patched function because tauri/webkit does weird promise polyfills
	export function confirm(): boolean | Promise<boolean>;
}

export * from './constants';
export * from './library';
export * from './hooks';
export * from './utils';
export * from './types';
