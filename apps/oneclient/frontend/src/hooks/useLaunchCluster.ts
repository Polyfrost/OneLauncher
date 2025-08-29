import { bindings } from '@/main';
import { toast } from '@/utils/toast';
import { getMessageFromError, isLauncherError } from '@onelauncher/common';
import { useNavigate } from '@tanstack/react-router';
import { useCallback } from 'react';

export function useLaunchCluster(): (clusterId: number | undefined | null) => void;
export function useLaunchCluster(clusterId: number | undefined | null): () => void;

export function useLaunchCluster(clusterId?: number | undefined | null) {
	const navigate = useNavigate();

	const launch = useCallback(
		async (id?: number | null) => {
			const targetId = clusterId ?? id; // prefer the one from hook arg

			if (!targetId)
				return;

			try {
				await bindings.core.launchCluster(targetId, null);

				navigate({
					to: '/app/cluster/process',
					search: { clusterId: targetId },
				});
			}
			catch (err) {
				if (isLauncherError(err))
					toast({
						type: 'error',
						title: 'Launch Error',
						message: getMessageFromError(err),
					});
			}
		},
		[clusterId, navigate],
	);

	return launch;
}
