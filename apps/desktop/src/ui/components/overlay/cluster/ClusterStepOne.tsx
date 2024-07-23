import { User03Icon } from '@untitled-theme/icons-solid';
import { Index, type JSX, createEffect, createSignal, on } from 'solid-js';
import type { ClusterStepProps } from './ClusterCreationModal';
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
		icon: <User03Icon />,
	},
];

export default function ClusterStepOne(props: ClusterStepProps) {
	const [selected, setSelected] = createSignal<number>();

	const check = () => {
		props.setCanGoForward(selected() !== undefined);
	};

	createEffect(check);
	createEffect(on(() => props.isVisible(), (curr: boolean) => {
		if (curr)
			check();
	}));

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
			<div class="[&>svg]:(w-8 h-8)">
				{props.icon}
			</div>
			<span>{props.name}</span>
		</div>
	);
}
