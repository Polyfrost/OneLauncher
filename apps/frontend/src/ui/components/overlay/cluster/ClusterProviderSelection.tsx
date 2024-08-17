import { Upload01Icon, User03Icon } from '@untitled-theme/icons-solid';
import { Index, type JSX, createEffect, createSignal } from 'solid-js';
import { type ClusterStepProps, CreationStage, createClusterStep } from './ClusterCreationModal';
import ModrinthIcon from '~assets/logos/modrinth.svg?component-solid';
import CurseforgeIcon from '~assets/logos/curseforge.svg?component-solid';

const providers: Omit<ProviderCardProps, 'selected' | 'setSelected'>[] = [
	{
		name: 'New',
		icon: <User03Icon />,
	},
	{
		name: 'Modrinth',
		icon: <ModrinthIcon color="#1bd96a" />,
	},
	{
		name: 'Curseforge',
		icon: <CurseforgeIcon color="#F16436" />,
	},
	{
		name: 'Import',
		icon: <Upload01Icon />,
	},
];

export default createClusterStep({
	message: 'Select Provider',
	buttonType: 'next',
	Component: ClusterProviderSelection,
});

function ClusterProviderSelection(props: ClusterStepProps) {
	const [selected, setSelected] = createSignal<number>();

	const check = () => {
		// TODO: Add more stages
		// eslint-disable-next-line solid/reactivity -- It's fine
		props.setCanGoForward(() => {
			const isTrue = selected() !== undefined;

			if (isTrue)
				props.setNextStage(CreationStage.GAME_SETUP);

			return isTrue;
		});
	};

	createEffect(check);

	return (
		<div class="grid grid-cols-3 gap-2">
			<Index each={providers}>
				{(provider, index) => (
					<ProviderCard
						{...provider()}
						selected={index === selected()}
						setSelected={() => setSelected(index)}
					/>
				)}
			</Index>
		</div>
	);
}

interface ProviderCardProps {
	icon: JSX.Element;
	name: string;
	setSelected: () => any;
	selected: boolean;
};

function ProviderCard(props: ProviderCardProps) {
	return (
		<div
			onClick={() => props.setSelected()}
			class={`flex flex-col justify-center items-center gap-y-2 py-2 px-4 hover:bg-component-bg-hover active:bg-component-bg-pressed rounded-lg ${props.selected ? 'bg-component-bg' : ''}`}
		>
			<div class="[&>svg]:(h-8 w-8)">
				{props.icon}
			</div>
			<span>{props.name}</span>
		</div>
	);
}
