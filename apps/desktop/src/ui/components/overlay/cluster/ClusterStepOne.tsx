import { User03Icon } from '@untitled-theme/icons-solid';
import { For, type JSX } from 'solid-js';
import ModrinthIcon from '~assets/logos/modrinth.svg?component-solid';
import CurseforgeIcon from '~assets/logos/curseforge.svg?component-solid';

const providers: ProviderCardProps[] = [
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
];

export default function ClusterStepOne() {
	return (
		<div class="grid grid-cols-3 gap-2">
			<For each={providers}>
				{provider => <ProviderCard {...provider} />}
			</For>
		</div>
	);
}

interface ProviderCardProps {
	icon: JSX.Element;
	name: string;
};

function ProviderCard(props: ProviderCardProps) {
	return (
		<div class="flex flex-col justify-center items-center gap-y-2 py-2 px-4 hover:bg-component-bg-hover active:bg-component-bg-pressed rounded-lg">
			<div class="[&>svg]:(w-8 h-8)">
				{props.icon}
			</div>
			<span>{props.name}</span>
		</div>
	);
}
