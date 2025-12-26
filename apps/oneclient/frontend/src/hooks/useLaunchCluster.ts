import { bindings } from '@/main';
import { toast } from '@/utils/toast';
import { getMessageFromError, isLauncherError } from '@onelauncher/common';
import { useNavigate } from '@tanstack/react-router';
import { useCallback, useState } from 'react';

export function useLaunchCluster(): { launch: (clusterId: number | undefined | null) => void; showDownloadWarning: boolean; setShowDownloadWarning: (show: boolean) => void; forceLaunch: () => void };
export function useLaunchCluster(clusterId: number | undefined | null): { launch: () => void; showDownloadWarning: boolean; setShowDownloadWarning: (show: boolean) => void; forceLaunch: () => void };

export function useLaunchCluster(clusterId?: number | undefined | null) {
	const navigate = useNavigate();
	const [showDownloadWarning, setShowDownloadWarning] = useState(false);
	const [lastAttemptedId, setLastAttemptedId] = useState<number | null>(null);

	const launch = useCallback(
		async (id?: number | null) => {
			const targetId = clusterId ?? id; // prefer the one from hook arg

			if (!targetId)
				return;

			setLastAttemptedId(targetId);

			try {
				await bindings.core.launchCluster(targetId, null, false);

				navigate({
					to: '/app/cluster/process',
					search: { clusterId: targetId },
				});
			}
			catch (err: any) {
				if (err.type === 'ClusterError' && err.data.type === 'ClusterDownloading') {
					setShowDownloadWarning(true);
					return;
				}

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

	const forceLaunch = useCallback(async () => {
		if (!lastAttemptedId)
			return;
		// @ts-ignore - ignoring
		await bindings.core.setClusterStage(lastAttemptedId, 'notready');
		setShowDownloadWarning(false);
		launch(lastAttemptedId);
	}, [lastAttemptedId, launch]);

	return { launch, showDownloadWarning, setShowDownloadWarning, forceLaunch };
}
