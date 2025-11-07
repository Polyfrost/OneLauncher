import type { ClusterModel, ManagedPackage, ManagedVersion } from '@/bindings.gen';
import { bindings } from '@/main';
import { useCommandMut } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { Download01Icon } from '@untitled-theme/icons-react';

export function DownloadModButton({ pkg, version, cluster }: { pkg: ManagedPackage; version: ManagedVersion; cluster: ClusterModel }) {
	const download = useCommandMut(() => bindings.core.downloadPackage(pkg.provider, pkg.id, version.version_id, cluster.id, true));

	const handleDownload = () => {
		(async () => {
			await download.mutateAsync();
		})();
	};

	return (
		<Button
			className="flex flex-col items-center justify-center"
			color="primary"
			onClick={handleDownload}
			size="iconLarge"
		>
			<Download01Icon />
		</Button>
	);
}
