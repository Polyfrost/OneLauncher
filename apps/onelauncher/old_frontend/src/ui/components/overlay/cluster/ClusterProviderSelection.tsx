import type { ImportType } from '@onelauncher/client/bindings';
import { User03Icon } from '@untitled-theme/icons-solid';
import LauncherIcon from '~ui/components/content/LauncherIcon';
import { LAUNCHER_IMPORT_TYPES } from '~utils';
import { createEffect, createSignal, For, type JSX } from 'solid-js';
import { type ClusterStepProps, createClusterStep, CreationStage } from './ClusterCreationModal';

export default createClusterStep({
	message: 'Select Provider',
	buttonType: 'next',
	Component: ClusterProviderSelection,
});

export type ClusterCreationProvider = ImportType | 'New';

function ClusterProviderSelection(props: ClusterStepProps) {
	const [selected, setSelected] = createSignal<number>();

	createEffect(() => {
		// eslint-disable-next-line solid/reactivity -- It's fine
		props.setCanGoForward(() => {
			const index = selected();
			if (index === undefined)
				return false;

			if (index === -1) {
				props.controller.setProvider('New');
				props.setNextStage(CreationStage.GAME_SETUP);
			}
			else if (index >= 0) {
				props.controller.setProvider(LAUNCHER_IMPORT_TYPES[index]);
				props.setNextStage(CreationStage.IMPORT_SELECTION);
			}

			return true;
		});
	});

	return (
		<div class="grid grid-cols-3 gap-2">
			<ProviderCard
				icon={<User03Icon />}
				name="New"
				selected={selected() === -1}
				setSelected={() => setSelected(-1)}
			/>

			<For each={LAUNCHER_IMPORT_TYPES}>
				{(provider, index) => (
					<ProviderCard
						importType={provider}
						selected={index() === selected()}
						setSelected={() => setSelected(index)}
					/>
				)}
			</For>
		</div>
	);
}

interface ProviderLauncherCardProps {
	importType: ImportType;
}

interface ProviderCustomCardProps {
	icon: JSX.Element;
	name: string;
}

type ProviderCardProps = (ProviderLauncherCardProps | ProviderCustomCardProps) & {
	setSelected: () => void;
	selected: boolean;
};

function ProviderCard(props: ProviderCardProps) {
	const Icon = () => {
		if ('icon' in props)
			// eslint-disable-next-line solid/components-return-once -- ok
			return props.icon;

		return <LauncherIcon launcher={props.importType} />;
	};

	const Name = () => {
		if ('name' in props)
			return props.name;

		return props.importType;
	};

	return (
		<div
			class={`flex flex-col justify-center items-center gap-y-3 py-2 px-4 hover:bg-component-bg-hover active:bg-component-bg-pressed rounded-lg ${props.selected ? 'bg-component-bg' : ''}`}
			onClick={() => props.setSelected()}
		>
			<div class="h-8 w-8 flex items-center justify-center [&>svg]:(w-8 h-8!)">
				<Icon />
			</div>
			<span>{Name()}</span>
		</div>
	);
}
