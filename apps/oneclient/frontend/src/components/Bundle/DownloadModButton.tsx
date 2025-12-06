import type { ClusterModel, ModpackFile } from '@/bindings.gen';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { useCommandMut } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { Download01Icon } from '@untitled-theme/icons-react';
import { twMerge } from 'tailwind-merge';

export function DownloadModButton({ cluster, file }: { cluster: ClusterModel; file: ModpackFile }) {
	const download = useCommandMut(async () => {
		if ('Managed' in file.kind) {
			const [pkg, version] = file.kind.Managed;
			if (version.dependencies.length > 0)
				for (const dependency of version.dependencies) {
					const slug = dependency.project_id ?? '';
					const versions = await bindings.core.getPackageVersions(pkg.provider, slug, cluster.mc_version, cluster.mc_loader, 0, 1);
					await bindings.core.downloadPackage(pkg.provider, slug, versions.items[0].version_id, cluster.id, null);
				}

			await bindings.core.downloadPackage(pkg.provider, pkg.id, version.version_id, cluster.id, null);
		}
		else {
			await bindings.core.downloadExternalPackage(file.kind.External, cluster.id, null, null);
		}
	});

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
