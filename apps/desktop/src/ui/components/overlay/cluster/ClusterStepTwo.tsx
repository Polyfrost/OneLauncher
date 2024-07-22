import { TextInputIcon } from '@untitled-theme/icons-solid';
import { For, type JSX, createEffect, createSignal, on, onMount, splitProps } from 'solid-js';
import type { ClusterStepProps } from './ClusterCreationModal';
import Dropdown from '~ui/components/base/Dropdown';
import TextField from '~ui/components/base/TextField';
import VanillaImage from '~assets/logos/vanilla.png';
import FabricImage from '~assets/logos/fabric.png';
import ForgeImage from '~assets/logos/forge.png';
import QuiltImage from '~assets/logos/quilt.png';

const loaders: {
	name: string;
	icon: () => JSX.Element;
}[] = [
	{
		name: 'Vanilla',
		icon: () => <img src={VanillaImage} />,
	},
	{
		name: 'Fabric',
		icon: () => <img src={FabricImage} />,
	},
	{
		name: 'Forge',
		icon: () => <img src={ForgeImage} />,
	},
	{
		name: 'Quilt',
		icon: () => <img src={QuiltImage} />,
	},
];

export function ClusterStepTwo(props: ClusterStepProps) {
	const [name, setName] = createSignal('');

	const check = () => {
		const hasName = name().length > 0;

		props.setCanGoForward(hasName);
	};

	createEffect(check);
	createEffect(on(() => props.isVisible(), (curr: boolean) => {
		if (curr)
			check();
	}));

	return (
		<div class="flex flex-col gap-y-4">
			<Option header="Name">
				<TextField
					onInput={e => setName(e.target.value)}
					placeholder="Name"
					iconLeft={<TextInputIcon />}
				/>
			</Option>

			<div class="flex flex-row gap-x-2">
				<Option class="flex-1" header="Versions">
					<Dropdown>
						<For each={Array.from({ length: 300 }, (_, i) => i).reverse()}>
							{version => (
								<Dropdown.Row>{version}</Dropdown.Row>
							)}
						</For>
					</Dropdown>
				</Option>
				<Option class="w-32" header="Loader">
					<Dropdown>
						<For each={loaders}>
							{loader => (
								<Dropdown.Row>
									<div class="flex flex-row gap-x-2">
										<div class="w-4 h-4">
											<loader.icon />
										</div>
										{loader.name}
									</div>
								</Dropdown.Row>
							)}
						</For>
					</Dropdown>
				</Option>
			</div>
		</div>
	);
}

type OptionProps = {
	header: string;
} & JSX.HTMLAttributes<HTMLDivElement>;

function Option(props: OptionProps) {
	const [split, rest] = splitProps(props, ['header', 'class']);

	return (
		<div {...rest} class={`flex flex-col gap-y-2 items-stretch ${split.class || ''}`}>
			<h3 class="text-md font-semibold uppercase text-fg-secondary text-left">{props.header}</h3>
			<div class="max-h-8">
				{props.children}
			</div>
		</div>
	);
}
