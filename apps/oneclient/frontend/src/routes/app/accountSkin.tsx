import type { MinecraftCredentials } from '@/bindings.gen';
import type { PlayerAnimation } from 'skinview3d';
import { SheetPage, SkinViewer } from '@/components';
import { ImportSkinModal, RemoveSkinCapeModal } from '@/components/overlay';
import { Overlay } from '@/components/overlay/Overlay';
import { usePlayerProfile } from '@/hooks/usePlayerProfile';
import { bindings } from '@/main';
import { getSkinUrl } from '@/utils/minecraft';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useQueryClient } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import { dataDir, downloadDir, join } from '@tauri-apps/api/path';
import { save } from '@tauri-apps/plugin-dialog';
import { exists, mkdir, readTextFile, writeFile, writeTextFile } from '@tauri-apps/plugin-fs';
import { Download01Icon, PlusIcon, Trash01Icon } from '@untitled-theme/icons-react';
import { useEffect, useState } from 'react';
import { DialogTrigger } from 'react-aria-components';
import { CrouchAnimation, FlyingAnimation, HitAnimation, IdleAnimation, WalkingAnimation } from 'skinview3d';

interface Skin {
	is_slim: boolean;
	skin_url: string;
}

interface Cape {
	url: string;
	id: string;
}

export const Route = createFileRoute('/app/accountSkin')({
	component: RouteComponent,
});

const animations = [
	{ name: 'Idle', animation: new IdleAnimation(), speed: 0.1 },
	{ name: 'Walking', animation: new WalkingAnimation(), speed: 0.1 },
	{ name: 'Flying', animation: new FlyingAnimation(), speed: 0.2 },
	{ name: 'Crouch', animation: new CrouchAnimation(), speed: 0.025 },
	{ name: 'Hit', animation: new HitAnimation(), speed: 0.1 },
];

async function getSkinHistory(): Promise<Array<Skin>> {
	const parentDir = await join(await dataDir(), 'OneClient', 'metadata', 'history');
	const skinsPath = await join(parentDir, 'skins.json');
	try {
		const dirExists = await exists(parentDir);
		if (!dirExists)
			await mkdir(parentDir, { recursive: true });

		const fileExists = await exists(skinsPath);
		if (!fileExists) {
			await writeTextFile(skinsPath, JSON.stringify([]));
			return [];
		}

		const contents = await readTextFile(skinsPath);
		return JSON.parse(contents) as Array<Skin>;
	}
	catch (error) {
		console.error(error);
		await writeTextFile(skinsPath, JSON.stringify([]));
		return [];
	}
}

async function saveSkinHistory(skins: Array<Skin>): Promise<void> {
	const parentDir = await join(await dataDir(), 'OneClient', 'metadata', 'history');
	const skinsPath = await join(parentDir, 'skins.json');
	try {
		const dirExists = await exists(parentDir);
		if (!dirExists)
			await mkdir(parentDir, { recursive: true });

		await writeTextFile(skinsPath, JSON.stringify(skins));
	}
	catch (error) {
		console.error(error);
	}
}

function RouteComponent() {
	const queryClient = useQueryClient();
	const [skins, setSkinsSTATE] = useState<Array<Skin>>([]);
	const setSkins = (updater: Array<Skin> | ((prev: Array<Skin>) => Array<Skin>)) => {
		const newSkins = typeof updater === 'function' ? updater(skins) : updater;

		const seen: Set<string> = new Set();
		const filteredDupes = newSkins.filter((skin) => {
			const key = skin.skin_url || '';
			if (seen.has(key))
				return false;
			seen.add(key);
			return true;
		});

		saveSkinHistory(filteredDupes);
		setSkinsSTATE(filteredDupes);
	};
	const [capes, setCapes] = useState<Array<Cape>>([]);
	const { data: currentAccount } = useCommandSuspense(['getDefaultUser'], () => bindings.core.getDefaultUser(true));
	const { data: loggedInUser } = useCommandSuspense(['fetchLoggedInProfile'], () => bindings.core.fetchLoggedInProfile((currentAccount as MinecraftCredentials).access_token));
	useEffect(() => {
		async function fetchSkins() {
			const skins: Array<Skin> = loggedInUser.skins.map(skin => ({
				is_slim: skin.variant === 'slim',
				skin_url: skin.url,
			}));
			(await getSkinHistory()).forEach(skin => skins.push(skin));
			setSkins(skins);
		}

		fetchSkins();

		setCapes([{ url: "", id: "" }, ...loggedInUser.capes.map(cape => ({ url: cape.url, id: cape.id }))]);
	}, [loggedInUser]);
	const { data: profile } = usePlayerProfile(currentAccount?.id);
	const [animation, setAnimation] = useState<PlayerAnimation>(animations[0].animation);
	const [animationName, setAnimationName] = useState<string>(animations[0].name);
	const skinData: Skin = {
		is_slim: profile?.is_slim ?? false,
		skin_url: getSkinUrl(profile?.skin_url),
	};
	const [selectedSkin, setSelectedSkin] = useState<Skin>(skinData);
	useEffect(() => {
		if (!skinData.skin_url)
			return;
		setSkins((prev) => {
			const filtered = prev.filter(skin => skin.skin_url !== skinData.skin_url);
			return [skinData, ...filtered];
		});
		setSelectedSkin(skinData);
	}, [skinData.skin_url, profile]);

	const [selectedCape, setSelectedCape] = useState<string>('');

	const importFromURL = (url: string) => {
		setSkins([...skins, { is_slim: false, skin_url: url }]);
	};
	const [shouldShowElytra, setShouldShowElytra] = useState<boolean>(false);

	const saveSkinToAccount = async () => {
		try {
			if (!currentAccount)
				return;
			await bindings.core.changeSkin(currentAccount.access_token, selectedSkin.skin_url, selectedSkin.is_slim ? 'slim' : 'classic');
			if (selectedCape === '') {
				await bindings.core.removeCape(currentAccount.access_token)
			} else {
				const capeData = capes.find((cape) => cape.url === selectedCape)
				if (!capeData) return
				await bindings.core.changeCape(currentAccount.access_token, capeData.id)
			}
			queryClient.invalidateQueries({
				queryKey: ['getDefaultUser'],
			});
			queryClient.invalidateQueries({
				queryKey: ['fetchLoggedInProfile'],
			});
			queryClient.invalidateQueries({
				queryKey: ['fetchMinecraftProfile'],
			});
		}
		catch (error) {
			console.error(error);
		}
	};

	if (currentAccount === null)
		return (
			<SheetPage headerLarge={<></>} headerSmall={<></>}>
				<SheetPage.Content>
					<p>No accounts added</p>
				</SheetPage.Content>
			</SheetPage>
		);

	const getNextAnimationData = () => {
		const animationIndex = animations.findIndex(animationData => animationData.name === animationName);
		if (animationIndex === -1 || animationIndex === animations.length - 1)
			return animations[0];
		else
			return animations[animationIndex + 1];
	};

	const changeSelectedAnimation = () => {
		const data = getNextAnimationData();
		data.animation.speed = data.speed;
		if (data.name === 'Walking')
			(data.animation as WalkingAnimation).headBobbing = false;
		setAnimation(data.animation);
		setAnimationName(data.name);
	};

	if (!selectedSkin.skin_url)
		return <></>;

	return (
		<SheetPage
			headerLarge={(
				<HeaderLarge save={saveSkinToAccount} username={profile?.username || 'UNKNOWN'} />
			)}
			headerSmall={<HeaderSmall />}
		>
			<SheetPage.Content>
				<div className="flex-1 flex flex-row gap-8">
					<div className="flex flex-col justify-center items-center">
						<p>Current Skin</p>
						<Button className="mt-2" color="ghost" onClick={changeSelectedAnimation}>
							<p>Change to {`${getNextAnimationData().name} Animation`}</p>
						</Button>

						<Viewer
							animation={animation}
							capeURL={selectedCape}
							enableControls
							shouldShowElytra={shouldShowElytra}
							skinURL={selectedSkin.skin_url}
						/>
					</div>

					<div className="min-h-full w-px bg-component-border"></div>

					<div className="w-full flex flex-col min-h-full justify-between">

						<SkinHistoryRow
							animation={animation}
							capeURL={selectedCape}
							importFromURL={importFromURL}
							selected={selectedSkin}
							setSelectedSkin={setSelectedSkin}
							setSkins={setSkins}
							shouldShowElytra={shouldShowElytra}
							skins={skins}
						/>

						<div className="min-w-full h-px bg-component-border"></div>

						<CapeRow
							animation={animation}
							capes={capes}
							selected={selectedCape}
							setSelectedCape={setSelectedCape}
							setShouldShowElytra={() => setShouldShowElytra(!shouldShowElytra)}
							shouldShowElytra={shouldShowElytra}
							skinURL={selectedSkin.skin_url}
						/>

					</div>

				</div>
			</SheetPage.Content>
		</SheetPage>
	);
}

function SkinHistoryRow({ selected, animation, setSelectedSkin, skins, setSkins, importFromURL, capeURL, shouldShowElytra }: { selected: Skin; animation: PlayerAnimation; setSelectedSkin: (skin: Skin) => void; skins: Array<Skin>; setSkins: React.Dispatch<React.SetStateAction<Array<Skin>>>; importFromURL: (url: string) => void; capeURL: string; shouldShowElytra: boolean }) {
	return (
		<div className="flex flex-col h-full justify-around">
			<div className="flex flex-col justify-center items-center">
				<p>Skin History</p>
			</div>

			<div className="flex flex-row h-fit gap-2">
				<DialogTrigger>
					<Button className="border border-component-border rounded-xl bg-component-border w-[75px] h-[120px]" color="ghost">
						<div className="flex flex-col justify-center items-center content-center h-full">
							<PlusIcon className="scale-200" />
						</div>
					</Button>
					<Overlay>
						<ImportSkinModal importFromURL={importFromURL} />
					</Overlay>
				</DialogTrigger>
				{skins.map(skinData => (
					<RenderSkin
						animation={animation}
						capeURL={capeURL}
						key={skinData.skin_url}
						selected={selected}
						setSelectedSkin={setSelectedSkin}
						setSkins={setSkins}
						shouldShowElytra={shouldShowElytra}
						skin={skinData}
					/>
				))}
			</div>
		</div>
	);
}

function RenderSkin({ skin, selected, animation, setSelectedSkin, setSkins, capeURL, shouldShowElytra }: { skin: Skin; selected: Skin; animation: PlayerAnimation; setSelectedSkin: (skin: Skin) => void; setSkins: React.Dispatch<React.SetStateAction<Array<Skin>>>; capeURL: string; shouldShowElytra: boolean }) {
	const exportSkin = async () => {
		try {
			if (!skin.skin_url)
				return;
			const filePath = await save({
				title: 'Skin Export Location',
				filters: [
					{
						name: 'Images',
						extensions: ['png'],
					},
				],
				defaultPath: await join(await downloadDir(), `${skin.skin_url.split('/').reverse()[0]}.png`),
			});

			if (!filePath)
				return;

			const response = await fetch(skin.skin_url);
			const buffer = await response.arrayBuffer();

			await writeFile(filePath, new Uint8Array(buffer));
		}
		catch (error) {
			console.error(error);
		}
	};
	if (!skin.skin_url)
		return <></>;
	return (
		<Button
			className={`w-[75px] h-[120px] relative border rounded-xl bg-component-border ${selected.skin_url === skin.skin_url ? 'border-brand' : 'hover:border-brand border-component-border'}`}
			color="ghost"
			onClick={() => setSelectedSkin(skin)}
		>
			<Viewer
				animation={animation}
				capeURL={capeURL}
				height={120}
				shouldShowElytra={shouldShowElytra}
				showText={false}
				skinURL={skin.skin_url}
				width={75}
			/>
			{selected.skin_url === skin.skin_url
				? <></>
				: (
					<DialogTrigger>
						<Button className="group w-8 h-8 absolute top-0 right-0" color="ghost" size="icon">
							<Trash01Icon className="group-hover:stroke-danger" />
						</Button>

						<Overlay>
							<RemoveSkinCapeModal onPress={() => setSkins(prev => prev.filter(skinData => skinData.skin_url !== skin.skin_url))} />
						</Overlay>
					</DialogTrigger>
				)}
			<Button
				className="group w-8 h-8 absolute bottom-0 right-0"
				color="ghost"
				onPress={exportSkin}
				size="icon"
			>
				<Download01Icon className="group-hover:stroke-brand" />
			</Button>
		</Button>
	);
}

function CapeRow({ selected, animation, setSelectedCape, capes, shouldShowElytra, setShouldShowElytra, skinURL }: { selected: string | null; animation: PlayerAnimation; setSelectedCape: (cape: string) => void; capes: Array<Cape>; shouldShowElytra: boolean; setShouldShowElytra: () => void; skinURL: string }) {
	return (
		<div className="flex flex-col h-full justify-around">
			<div className="flex flex-row h-fit gap-2">
				{capes.map(cape => (
					<RenderCape
						animation={animation}
						cape={cape.url}
						key={cape.id}
						selected={selected}
						setSelectedCape={setSelectedCape}
						shouldShowElytra={shouldShowElytra}
						skinURL={skinURL}
					/>
				))}

			</div>

			<div className="flex flex-col justify-center items-center">
				<p>Cape History</p>
				<Button color="ghost" onClick={setShouldShowElytra}>
					<p>{`${shouldShowElytra ? 'Disable' : 'Enable'} Elytra`}</p>
				</Button>
			</div>
		</div>
	);
}

function RenderCape({ selected, animation, setSelectedCape, cape, shouldShowElytra, skinURL }: { selected: string | null; animation: PlayerAnimation; setSelectedCape: (cape: string) => void; cape: string; shouldShowElytra: boolean; skinURL: string }) {
	return (
		<Button
			className={`w-[75px] h-[120px] relative border rounded-xl bg-component-border ${selected === cape ? 'border-brand' : 'hover:border-brand border-component-border'}`}
			color="ghost"
			onClick={() => setSelectedCape(cape)}
		>
			<Viewer
				animation={animation}
				capeURL={cape}
				flip
				height={120}
				shouldShowElytra={shouldShowElytra}
				showText={false}
				skinURL={skinURL}
				width={75}
			/>
		</Button>
	);
}

function HeaderLarge({ username, save }: { username: string; save: () => void }) {
	return (
		<div className="flex flex-row justify-between items-end gap-16">
			<div className="flex-1 flex flex-row justify-between">
				<h1 className="text-3xl font-semibold">{`${username}'s Skins`}</h1>

				<div className="flex flex-row gap-2">
					<Button color="primary" onClick={save} size="large">
						<p>Save</p>
					</Button>
				</div>
			</div>
		</div>
	);
}

function HeaderSmall() {
	return (
		<div className="flex flex-row justify-between items-center h-full">
			<h1 className="text-2lg h-full font-medium">Accounts</h1>
		</div>
	);
}

function Viewer({ skinURL, capeURL, height = 400, width = 250, showText = true, animation, enableControls = false, flip = false, shouldShowElytra }: { skinURL: string; capeURL: string; height?: number; width?: number; showText?: boolean; animation?: PlayerAnimation; enableControls?: boolean; flip?: boolean; shouldShowElytra: boolean }) {
	return (
		<SkinViewer
			animate
			animation={animation}
			autoRotate={false}
			capeUrl={capeURL === '' ? null : capeURL}
			className="h-full w-full max-w-1/4"
			elytra={shouldShowElytra}
			enableDamping={enableControls}
			enablePan={enableControls}
			enableRotate={enableControls}
			enableZoom={enableControls}
			height={height}
			playerRotateTheta={(-Math.PI / 6) - (flip ? Math.PI : 0)}
			showText={showText}
			skinUrl={skinURL}
			translateRotateY={-2}
			width={width}
			zoom={0.8}
		/>
	);
}
