import type { ModData, ModDataArray } from '../DownloadMods';
import { bindings } from '@/main';
import { useCommandMut } from '@onelauncher/common';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useState } from 'react';
import { isManagedMod } from '../DownloadMods';
import { Overlay } from './Overlay';

function downloadModsParallel(items: ModDataArray, limit: number, fn: (mod: ModData, index: number) => Promise<void>) {
	let index = 0;
	const workers = Array.from({ length: limit }).fill(null).map(async () => {
		while (index < items.length) {
			const i = index++;
			await fn(items[i], i);
		}
	});
	return Promise.all(workers);
}

export function DownloadingMods({ mods, setOpen, nextPath }: { mods: ModDataArray; setOpen: React.Dispatch<React.SetStateAction<boolean>>; nextPath: string }) {
	const navigate = useNavigate();
	const [downloadedMods, setDownloadedMods] = useState(0);
	const [modName, setModName] = useState<string | null>(null);
	const download = useCommandMut(async (mod: ModData) => {
		if (isManagedMod(mod))
			await bindings.core.downloadPackage(mod.provider, mod.id, mod.versionId, mod.clusterId, true);
		else
			await bindings.core.downloadExternalPackage(mod.package, mod.clusterId, null, null);
	});

	useEffect(() => {
		const downloadAll = async () => {
			await downloadModsParallel(mods, 25, async (mod) => {
				setModName(mod.name);
				try {
					await download.mutateAsync(mod);
				}
				finally {
					setDownloadedMods(prev => prev + 1);
				}
			});
		};

		downloadAll();
	}, [mods]);

	useEffect(() => {
		if (downloadedMods >= mods.length) {
			setOpen(false);
			navigate({ to: nextPath });
		}
	}, [downloadedMods, mods, navigate, nextPath, setOpen]);

	return (
		<Overlay.Dialog isDismissable={false}>
			<Overlay.Title>Downloading Mods</Overlay.Title>

			<div className="w-full flex flex-col items-center gap-2">
				<p>Downloaded {downloadedMods} / {mods.length}</p>
				<div className="w-1/2 h-4 bg-component-bg-disabled rounded-full outline-2 outline-ghost-overlay">
					<div
						className="h-full bg-brand rounded-full transition-all duration-300"
						style={{ width: mods.length > 0 ? `${(downloadedMods / mods.length) * 100}%` : '0%' }}
					>
					</div>
				</div>
				{modName !== null ? <p>Downloading {modName}</p> : <></>}
			</div>

		</Overlay.Dialog>
	);
}
