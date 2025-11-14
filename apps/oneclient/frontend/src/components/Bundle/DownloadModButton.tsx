import type { ClusterModel, ManagedPackage, ManagedVersion } from '@/bindings.gen';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { useCommandMut } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { Download01Icon } from '@untitled-theme/icons-react';
import { twMerge } from 'tailwind-merge';

export function DownloadModButton({ pkg, version, cluster }: { pkg: ManagedPackage; version: ManagedVersion; cluster: ClusterModel }) {
	const download = useCommandMut(() => bindings.core.downloadPackage(pkg.provider, pkg.id, version.version_id, cluster.id, true));

	const handleDownload = () => {
		(async () => {
			await download.mutateAsync();
		})();
	};

	const { setting } = useSettings();
	const useGridLayout = setting('mod_list_use_grid');

	return (
		<Button
			className={twMerge('flex flex-col items-center justify-center', useGridLayout ? 'w-full' : '')}
			color="primary"
			onClick={handleDownload}
			size={useGridLayout ? 'large' : 'iconLarge'}
		>
			<Download01Icon />
		</Button>
	);
}
