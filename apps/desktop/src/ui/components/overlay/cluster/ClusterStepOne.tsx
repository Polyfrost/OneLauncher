import type { JSX } from 'solid-js';

export default function ClusterStepOne() {
	return (
		<div class="grid grid-cols-3">
			helo
		</div>
	);
}

interface ProviderCardProps {
	icon: () => JSX.Element;
	name: string;
};

function ProviderCard(props: ProviderCardProps) {
	return (
		<div class="flex flex-col justify-center items-center gap-y-2">
			<props.icon />
			<span>{props.name}</span>
		</div>
	);
}
