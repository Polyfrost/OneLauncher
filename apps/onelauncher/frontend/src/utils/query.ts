import { QueryClient } from '@tanstack/solid-query';

export const queryClient = new QueryClient({
	defaultOptions: {
		queries: {
			networkMode: 'always',
		},
		mutations: {
			networkMode: 'always',
		},
	},
});
