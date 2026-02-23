import type { ClusterModel, ManagedVersion, ModpackArchive, ModpackFile, PackageModel, PackageType } from '@/bindings.gen';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { useMemo } from 'react';

/** Finds packages that are installed in the cluster but not part of any bundle, filtered by type. */
export function findCustomPackages(
	bundles: Array<ModpackArchive>,
	installedPackages: Array<PackageModel>,
	packageType: PackageType,
	excludeDependencies: boolean = false,
): Array<PackageModel> {
	const bundleKeys: Set<string> = new Set();
	const bundleHashes: Set<string> = new Set();
	// Collect dependency project IDs to exclude hidden bundle deps (mods only).
	// e.g. fabric-language-kotlin is hidden in the PolyMrPack manifest but
	// listed as a required dependency of visible bundle mods.
	const bundleDependencyIds: Set<string> = new Set();

	bundles.forEach((b) => {
		b.manifest.files.forEach((f) => {
			if ('Managed' in f.kind) {
				bundleKeys.add(`${f.kind.Managed[0].provider}:${f.kind.Managed[0].id}`);
				if (excludeDependencies)
					f.kind.Managed[1].dependencies.forEach((dep) => {
						if (dep.project_id != null)
							bundleDependencyIds.add(dep.project_id);
					});
			}
			else {
				bundleHashes.add(f.kind.External.sha1);
			}
		});
	});

	return installedPackages.filter((pkg) => {
		if (pkg.package_type !== packageType)
			return false;
		if (bundleKeys.has(`${pkg.provider}:${pkg.package_id}`) || bundleHashes.has(pkg.hash))
			return false;
		if (excludeDependencies && bundleDependencyIds.has(pkg.package_id))
			return false;
		return true;
	});
}

/**
 * Builds a synthetic "[Custom]" ModpackArchive for installed packages that are
 * not part of any bundle. Batch-fetches Modrinth metadata so icons/descriptions
 * are available.
 */
export function useCustomBundle(
	bundles: Array<ModpackArchive>,
	installedPackages: Array<PackageModel>,
	cluster: ClusterModel,
	packageType: PackageType,
): ModpackArchive | null {
	const customInstalledPackages = useMemo(
		() => findCustomPackages(bundles, installedPackages, packageType, packageType === 'mod'),
		[bundles, installedPackages, packageType],
	);

	const modrinthIds = useMemo(
		() => customInstalledPackages.filter(p => p.provider === 'Modrinth').map(p => p.package_id),
		[customInstalledPackages],
	);

	const { data: fetchedManagedPackages } = useCommandSuspense(
		['getMultiplePackages', 'Modrinth', modrinthIds],
		() => bindings.core.getMultiplePackages('Modrinth', modrinthIds),
	);

	return useMemo<ModpackArchive | null>(() => {
		if (customInstalledPackages.length === 0)
			return null;

		const managedPackageMap = new Map(fetchedManagedPackages.map(mp => [mp.id, mp]));

		const files: Array<ModpackFile> = customInstalledPackages.map((pkg) => {
			const managed = managedPackageMap.get(pkg.package_id);
			if (managed) {
				const stubVersion: ManagedVersion = {
					version_id: pkg.version_id,
					project_id: pkg.package_id,
					display_name: pkg.display_name,
					display_version: pkg.display_version,
					changelog: null,
					dependencies: [],
					mc_versions: pkg.mc_versions,
					release_type: 'release',
					loaders: pkg.mc_loader,
					published: pkg.published_at,
					downloads: 0,
					files: [{ sha1: pkg.hash, url: '', file_name: pkg.file_name, primary: true, size: 0 }],
				};
				return { enabled: true, kind: { Managed: [managed, stubVersion] as [typeof managed, ManagedVersion] }, overrides: null };
			}
			return {
				enabled: true,
				kind: { External: { name: pkg.file_name, url: '', sha1: pkg.hash, size: 0, package_type: pkg.package_type } },
				overrides: { name: pkg.display_name, icon: pkg.icon ?? null, authors: null, description: null },
			};
		});

		return {
			path: '__custom__',
			format: 'PolyMrPack',
			manifest: {
				name: '[Custom]',
				version: '0.0.0',
				loader: cluster.mc_loader,
				loader_version: cluster.mc_loader_version ?? '',
				mc_version: cluster.mc_version,
				enabled: true,
				files,
			},
		};
	}, [customInstalledPackages, fetchedManagedPackages, cluster]);
}
