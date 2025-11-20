import type { Provider } from '@/bindings.gen';
import { bindings } from '@/main';
import { useCommandMut } from '@onelauncher/common';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useState } from 'react';
import { Overlay } from './Overlay';

interface ModData {
	name: string;
	provider: Provider;
	id: string;
	versionId: string;
	clusterId: number;
}

export function DownloadingMods({ mods, setOpen, nextPath }: { mods: Array<ModData>; setOpen: React.Dispatch<React.SetStateAction<boolean>>; nextPath: string }) {
	const navigate = useNavigate();
	const [downloadedMods, setDownloadedMods] = useState(0);
	const [modNames, setModNames] = useState<Array<string>>([]);

	const download = useCommandMut(async (mod: ModData) => {
		await bindings.core.downloadPackage(mod.provider, mod.id, mod.versionId, mod.clusterId, true);
	});

	useEffect(() => {
		const downloadAll = async () => {
			const groupSize = 15;
			for (let i = 0; i < mods.length; i += groupSize) {
				const group = mods.slice(i, i + groupSize);
				setModNames(prev => [...prev, ...group.map(mod => mod.name).filter(name => !prev.includes(name))]);

				try {
					await Promise.all(
						group.map(async (mod) => {
							try {
								await download.mutateAsync(mod);
								setDownloadedMods(prev => prev + 1);
							}
							finally {
								setModNames(prev => prev.filter(name => name !== mod.name));
							}
						}),
					);
				}
				catch (error) {
					console.error(error);
				}
			}
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
				{modNames.length > 0 ? modNames.map(modName => <p key={modName}>Downloading {modName}</p>) : <></>}
			</div>

		</Overlay.Dialog>
	);
}
