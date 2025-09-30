import type { MinecraftCredentials } from '@/bindings.gen';
import type { PlayerAnimation } from 'skinview3d';
import { SheetPage, SkinViewer } from '@/components';
import { Overlay } from '@/components/overlay/Overlay';
import { usePlayerProfile } from '@/hooks/usePlayerProfile';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { Download01Icon, PlusIcon, Trash01Icon } from '@untitled-theme/icons-react';
import { useEffect, useState } from 'react';
import { DialogTrigger } from 'react-aria-components';
import { CrouchAnimation, FlyingAnimation, HitAnimation, IdleAnimation, WalkingAnimation } from 'skinview3d';
import { ImportSkinModal, RemoveSkinCapeModal } from '../../components/overlay';

interface Skin {
	is_slim?: boolean;
	skin_url?: string | null;
	cape_url?: string | null;
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

function RouteComponent() {
	const [skins, setSkins] = useState<Array<Skin>>([]);
	const [capes, setCapes] = useState<Array<string>>([]);
	const { data: currentAccount } = useCommandSuspense(['getDefaultUser'], () => bindings.core.getDefaultUser(true));
	const { data: loggedInUser } = useCommandSuspense(['fetchLoggedInProfile'], () => bindings.core.fetchLoggedInProfile((currentAccount as MinecraftCredentials).access_token));
	useEffect(() => {
		setSkins(
			loggedInUser.skins.map(skin => ({
				is_slim: skin.variant === 'slim',
				skin_url: skin.url,
			})),
		);

		setCapes(loggedInUser.capes.map(cape => cape.url));
	}, [loggedInUser]);
	const { data: profile } = usePlayerProfile(currentAccount?.id);
	const [animation, setAnimation] = useState<PlayerAnimation>(animations[0].animation);
	const [animationName, setAnimationName] = useState<string>(animations[0].name);
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

	const [selectedCape, setSelectedCapeSTATE] = useState<string>(profile?.cape_url || '');
	const setSelectedCape = (cape: string) => {
		setSelectedCapeSTATE(cape);
		if (selectedSkin.skin_url !== null)
			setSelectedSkin({ ...selectedSkin, cape_url: cape });
	};
	useEffect(() => {
		setCapes((prev) => {
			const filtered = prev.filter(cape => cape !== '');
			return ['', ...filtered];
		});
		setSelectedCape(skinData.cape_url || '');
	}, [skinData.cape_url, profile]);

	const importFromURL = (url: string) => {
		setSkins([...skins, { is_slim: false, skin_url: url }]);
	};

	useEffect(() => {
		importFromURL('http://textures.minecraft.net/texture/90b8789136facaa9f87b765140e1c8135e6652f513481bd84e6bd8c44844d7ce');
	}, []);

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

	return (
		<SheetPage
			headerLarge={(
				<HeaderLarge username={profile?.username || 'UNKNOWN'} />
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
							enableControls
							skinData={selectedSkin}
						/>
					</div>

					<div className="min-h-full w-px bg-component-border"></div>

					<div className="w-full flex flex-col min-h-full justify-between">

						<SkinHistoryRow
							animation={animation}
							importFromURL={importFromURL}
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

function SkinHistoryRow({ selected, animation, setSelectedSkin, skins, setSkins, importFromURL }: { selected: Skin; animation: PlayerAnimation; setSelectedSkin: (skin: Skin) => void; skins: Array<Skin>; setSkins: React.Dispatch<React.SetStateAction<Array<Skin>>>; importFromURL: (url: string) => void }) {
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
		<Button
			className={`w-[75px] h-[120px] relative border rounded-xl bg-component-border ${selected.skin_url === skin.skin_url ? 'border-brand' : 'hover:border-brand border-component-border'}`}
			color="ghost"
			onClick={() => setSelectedSkin(skin)}
		>
			<Viewer
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
		</Button>
	);
}

function CapeRow({ selected, selectedSkin, animation, setSelectedCape, capes }: { selected: string | null; selectedSkin: Skin; animation: PlayerAnimation; setSelectedCape: (cape: string) => void; capes: Array<string> }) {
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

function RenderCape({ selected, selectedSkin, animation, setSelectedCape, cape }: { selected: string | null; selectedSkin: Skin; animation: PlayerAnimation; setSelectedCape: (cape: string) => void; cape: string }) {
	return (
		<Button
			className={`w-[75px] h-[120px] relative border rounded-xl bg-component-border ${selected === cape ? 'border-brand' : 'hover:border-brand border-component-border'}`}
			color="ghost"
			onClick={() => setSelectedCape(cape)}
		>
			<Viewer
				animation={animation}
				flip
				height={120}
				showText={false}
				skinData={{ ...selectedSkin, cape_url: cape }}
				width={75}
			/>
		</Button>
	);
}

function HeaderLarge({ username }: { username: string }) {
	return (
		<div className="flex flex-row justify-between items-end gap-16">
			<div className="flex-1 flex flex-row justify-between">
				<h1 className="text-3xl font-semibold">{`${username}'s Skins`}</h1>

				<div className="flex flex-row gap-2">
					<Button color="primary" size="large">
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

function Viewer({ skinData, height = 400, width = 250, showText = true, animation, enableControls = false, flip = false }: { skinData: Skin; height?: number; width?: number; showText?: boolean; animation?: PlayerAnimation; enableControls?: boolean; flip?: boolean }) {
	return (
		<SkinViewer
			animate
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
