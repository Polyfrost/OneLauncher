import type { MojangPlayerProfile } from '@/bindings.gen';
import type { QueryKey } from '@tanstack/react-query';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { useQueryClient } from '@tanstack/react-query';

type PartialUuid = string | undefined | null;

export function usePlayerProfile(uuid: (() => PartialUuid) | PartialUuid) {
	const id = typeof uuid === 'function' ? uuid() : uuid;
	const queryKey: QueryKey = ['fetchMinecraftProfile', id];

	const queryClient = useQueryClient();
	const state = queryClient.getQueryState(queryKey);

	const shouldFetch = (state?.fetchFailureCount ?? 0) < 1;

	return useCommand<MojangPlayerProfile>(
		queryKey,
		() => {
			if (!id)
				throw new Error('No player ID provided');

			return bindings.core.fetchMinecraftProfile(id);
		},
		{
			placeholderData: {
				username: 'Player',
				uuid: '00000000-0000-0000-0000-000000000000',
				is_slim: false,
				cape_url: null,
				skin_url: null,
			},
			enabled: shouldFetch,
			staleTime: Infinity,
		},
	);
}
