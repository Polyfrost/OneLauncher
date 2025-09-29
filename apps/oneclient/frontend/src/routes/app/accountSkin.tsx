import type { SkinVariant } from '@/bindings.gen';
import type { PlayerAnimation } from 'skinview3d';
import { SheetPage, SkinViewer } from '@/components';
import { Overlay } from '@/components/overlay/Overlay';
import { usePlayerProfile } from '@/hooks/usePlayerProfile';
import { bindings } from '@/main';
import { useCommandMut, useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useQueryClient } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import { PlusIcon, Trash01Icon } from '@untitled-theme/icons-react';
import { useEffect, useState } from 'react';
import { DialogTrigger } from 'react-aria-components';
import { IdleAnimation, WalkingAnimation } from 'skinview3d';
import { ImportSkinModal, RemoveSkinCapeModal } from '../../components/overlay';

interface Skin {
	is_slim?: boolean;
	skin_url?: string | null;
	cape_url?: string | null;
}

const skinsData: Array<Skin> = [
	{
		is_slim: false,
		skin_url: 'http://textures.minecraft.net/texture/90b8789136facaa9f87b765140e1c8135e6652f513481bd84e6bd8c44844d7ce',
	},
	{
		is_slim: true,
		skin_url: 'https://textures.minecraft.net/texture/69655f89a9fb19a7da292a757be65c52efeab337e3f9579ae090815cfe9cd6d5',
	},
	{
		is_slim: false,
		skin_url: 'http://textures.minecraft.net/texture/c8ccb0647686d04135ac92f4c19b9961b409f8ae3ac5dbea4040e57cda2bcaba',
	},
];

const capesData: Array<string> = [
	'http://textures.minecraft.net/texture/2340c0e03dd24a11b15a8b33c2a7e9e32abb2051b2481d0ba7defd635ca7a933',
	'http://textures.minecraft.net/texture/f9a76537647989f9a0b6d001e320dac591c359e9e61a31f4ce11c88f207f0ad4',
	'http://textures.minecraft.net/texture/28de4a81688ad18b49e735a273e086c18f1e3966956123ccb574034c06f5d336',
];

export const Route = createFileRoute('/app/accountSkin')({
	component: RouteComponent,
});

const idleAnimation = new IdleAnimation();
idleAnimation.speed = 0.15;
const walkingAnimation = new WalkingAnimation();
walkingAnimation.speed = 0.1;
walkingAnimation.headBobbing = false;

function RouteComponent() {
	const queryClient = useQueryClient();
	const [skins, setSkins] = useState<Array<Skin>>(skinsData);
	const [capes, setCapes] = useState<Array<string>>(capesData);
	const { data: currentAccount } = useCommandSuspense(['getDefaultUser'], () => bindings.core.getDefaultUser(true));
	const { data: profile } = usePlayerProfile(currentAccount?.id);
	const [isAnimated, setAnimated] = useState<boolean>(false);
	const [animation, setAnimation] = useState<PlayerAnimation>(idleAnimation);
	const skinData: Skin = {
		is_slim: profile?.is_slim,
		skin_url: profile?.skin_url,
		cape_url: profile?.cape_url,
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

	const [selectedCape, setSelectedCapeSTATE] = useState<string | null>(profile?.cape_url || null);
	const setSelectedCape = (cape: string | null) => {
		setSelectedCapeSTATE(cape);
		if (selectedSkin.skin_url !== null)
			setSelectedSkin({ ...selectedSkin, cape_url: cape });
	};
	useEffect(() => {
		if (!skinData.cape_url)
			return;
		setCapes((prev) => {
			const filtered = prev.filter(cape => cape !== skinData.cape_url);
			return ['', skinData.cape_url, ...filtered].filter(Boolean) as Array<string>;
		});
		setSelectedCape(skinData.cape_url);

		setCapes(['', ...capes]);
	}, [skinData.cape_url, profile]);

	const importFromURL = (url: string) => {
		setSkins([...skins, { is_slim: false, skin_url: url }]);
	};

	const { mutate: getLoggedInUser } = useCommandMut(bindings.core.fetchLoggedInProfile, {
		onSuccess() {
			queryClient.invalidateQueries({
				queryKey: ['getDefaultUser'],
			});
		},
	});

	const saveSkin = () => {
		if (!currentAccount)
			return;
		const token = getLoggedInUser(currentAccount.access_token);
		console.log(currentAccount);
	};

	const toggleAnimation = () => {
		setAnimation(isAnimated ? idleAnimation : walkingAnimation);
		setAnimated(!isAnimated);
	};

	return (
		<SheetPage
			headerLarge={(
				<HeaderLarge
					importFromURL={importFromURL}
					saveSkin={saveSkin}
					username={profile?.username || 'UNKNOWN'}
				/>
			)}
			headerSmall={<HeaderSmall />}
		>
			<SheetPage.Content>
				<div className="flex-1 flex flex-row gap-8">
					<div className="flex flex-col justify-center items-center">
						<p>Current Skin</p>
						<Viewer
							animate
							animation={animation}
							enableControls
							skinData={selectedSkin}
						/>
						<Button
							className="mt-2"
							color="secondary"
							onClick={toggleAnimation}
							size="large"
						>
							<p>{`${isAnimated ? 'Stop' : 'Start'} Walking Animation`}</p>
						</Button>
					</div>

					<div className="min-h-full w-px bg-component-border"></div>

					<div className="w-full flex flex-col min-h-full justify-between">

						<SkinHistoryRow
							animation={animation}
							selected={selectedSkin}
							setSelectedSkin={setSelectedSkin}
							setSkins={setSkins}
							skins={skins}
						/>

						<div className="min-w-full h-px bg-component-border"></div>

						<CapeRow
							animation={animation}
							capes={capes}
							selected={selectedCape}
							selectedSkin={selectedSkin}
							setSelectedCape={setSelectedCape}
						/>

					</div>

				</div>
			</SheetPage.Content>
		</SheetPage>
	);
}

function SkinHistoryRow({ selected, animation, setSelectedSkin, skins, setSkins }: { selected: Skin; animation: PlayerAnimation; setSelectedSkin: (skin: Skin) => void; skins: Array<Skin>; setSkins: React.Dispatch<React.SetStateAction<Array<Skin>>> }) {
	return (
		<div className="flex flex-col h-full justify-around">
			<div className="flex flex-col justify-center items-center">
				<p>Skin History</p>
			</div>

			<div className="flex flex-row h-fit gap-2">
				<div className="border border-component-border rounded-xl bg-component-border w-[75px] h-[120px]" key="newSkin">
					<div className="flex flex-col justify-center items-center content-center h-full">
						<PlusIcon className="scale-200" />
					</div>
				</div>
				{skins.map(skinData => (
					<RenderSkin
						animation={animation}
						key={skinData.skin_url}
						selected={selected}
						setSelectedSkin={setSelectedSkin}
						setSkins={setSkins}
						skin={skinData}
					/>
				))}
			</div>
		</div>
	);
}

function RenderSkin({ skin, selected, animation, setSelectedSkin, setSkins }: { skin: Skin; selected: Skin; animation: PlayerAnimation; setSelectedSkin: (skin: Skin) => void; setSkins: React.Dispatch<React.SetStateAction<Array<Skin>>> }) {
	return (
		<div
			className={`relative border rounded-xl bg-component-border ${selected.skin_url === skin.skin_url ? 'border-brand' : 'hover:border-brand border-component-border'}`}
			onClick={() => setSelectedSkin(skin)}
		>
			<Viewer
				animate
				animation={animation}
				height={120}
				showText={false}
				skinData={skin}
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
		</div>
	);
}

function CapeRow({ selected, selectedSkin, animation, setSelectedCape, capes }: { selected: string | null; selectedSkin: Skin; animation: PlayerAnimation; setSelectedCape: (cape: string | null) => void; capes: Array<string> }) {
	return (
		<div className="flex flex-col h-full justify-around">
			<div className="flex flex-row h-fit gap-2">
				{capes.map(cape => (
					<RenderCape
						animation={animation}
						cape={cape}
						key={cape}
						selected={selected}
						selectedSkin={selectedSkin}
						setSelectedCape={setSelectedCape}
					/>
				))}

			</div>

			<div className="flex flex-col justify-center items-center">
				<p>Cape History</p>
			</div>
		</div>
	);
}

function RenderCape({ selected, selectedSkin, animation, setSelectedCape, cape }: { selected: string | null; selectedSkin: Skin; animation: PlayerAnimation; setSelectedCape: (cape: string | null) => void; cape: string }) {
	return (
		<div
			className={`relative border rounded-xl bg-component-border ${selected === cape ? 'border-brand' : 'hover:border-brand border-component-border'}`}
			onClick={() => setSelectedCape(cape)}
		>
			<Viewer
				animate
				animation={animation}
				flip
				height={120}
				showText={false}
				skinData={{ ...selectedSkin, cape_url: cape }}
				width={75}
			/>
		</div>
	);
}

function HeaderLarge({ username, importFromURL, saveSkin }: { username: string; importFromURL: (url: string) => void; saveSkin: () => void }) {
	return (
		<div className="flex flex-row justify-between items-end gap-16">
			<div className="flex-1 flex flex-row justify-between">
				<h1 className="text-3xl font-semibold">{`${username}'s Skins`}</h1>

				<div className="flex flex-row gap-2">
					<Button color="primary" onClick={saveSkin} size="large">
						<p>Save</p>
					</Button>
					<DialogTrigger>
						<Button color="secondary" size="large">
							<p>Import</p>
						</Button>

						<Overlay>
							<ImportSkinModal importFromURL={importFromURL} />
						</Overlay>
					</DialogTrigger>
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

function Viewer({ skinData, height = 400, width = 250, showText = true, animate = false, animation = idleAnimation, enableControls = false, flip = false }: { skinData: Skin; height?: number; width?: number; showText?: boolean; animate?: boolean; animation?: PlayerAnimation; enableControls?: boolean; flip?: boolean }) {
	return (
		<SkinViewer
			animate={animate}
			animation={animation}
			autoRotate={false}
			capeUrl={skinData.cape_url}
			className="h-full w-full max-w-1/4"
			enableDamping={enableControls}
			enablePan={enableControls}
			enableRotate={enableControls}
			enableZoom={enableControls}
			height={height}
			playerRotateX={Math.PI / 6}
			playerRotateY={Math.PI / 4 + (flip ? Math.PI : 0)}
			showText={showText}
			skinUrl={skinData.skin_url}
			translateRotateY={-2}
			width={width}
			zoom={0.8}
		/>
	);
}
