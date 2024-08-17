import { TextInputIcon } from '@untitled-theme/icons-solid';
import { For, Index, type JSX, Show, createEffect, createSignal, onMount, splitProps, untrack } from 'solid-js';
import { type ClusterStepProps, createClusterStep } from './ClusterCreationModal';
import Dropdown from '~ui/components/base/Dropdown';
import TextField from '~ui/components/base/TextField';
import useCommand from '~ui/hooks/useCommand';
import { bridge } from '~imports';
import SelectList from '~ui/components/base/SelectList';
import Checkbox from '~ui/components/base/Checkbox';
import type { Loader, VersionType } from '~bindings';
import { LOADERS, formatVersionRelease } from '~utils';
import LoaderIcon from '~ui/components/game/LoaderIcon';

export default createClusterStep({
	message: 'Game Setup',
	buttonType: 'create',
	Component: ClusterGameSetup,
});

function ClusterGameSetup(props: ClusterStepProps) {
	// const check = () => {
	// 	const hasName = (props.controller().partialCluster().name?.length ?? 0) > 0;
	// 	const hasVersion = (props.controller().partialCluster().mc_version?.length ?? 0) > 0;
	// 	const hasLoader = (props.controller().partialCluster().mod_loader?.length ?? 0) > 0;

	// 	props.setCanGoForward(hasName && hasVersion && hasLoader);
	// };

	// createEffect(check);
	// createEffect(on(() => props.isVisible(), (curr: boolean) => {
	// 	if (curr)
	// 		check();
	// }));

	const setName = (name: string) => props.controller.updatePartialCluster('name', name);
	const setVersion = (version: string) => props.controller.updatePartialCluster('mc_version', version);
	const setLoader = (loader: Loader | string) => props.controller.updatePartialCluster('mod_loader', loader.toLowerCase() as Loader);

	onMount(() => {
		setLoader('vanilla');
	});

	return (
		<div class="flex flex-col gap-y-4">
			<Option header="Name">
				<TextField
					onInput={e => setName(e.target.value)}
					placeholder="Name"
					iconLeft={<TextInputIcon />}
				/>
			</Option>

			<Option header="Versions">
				<VersionSelector setVersion={setVersion} />
			</Option>

			<Option header="Loader">
				<Dropdown onChange={index => setLoader(LOADERS[index] || 'vanilla')}>
					<For each={LOADERS}>
						{loader => (
							<Dropdown.Row>
								<div class="flex flex-row gap-x-2">
									<div class="h-4 w-4">
										<LoaderIcon loader={loader} />
									</div>
									{loader}
								</div>
							</Dropdown.Row>
						)}
					</For>
				</Dropdown>
			</Option>
		</div>
	);
}

type VersionReleaseFilters = {
	[key in VersionType]: boolean;
};

function VersionSelector(props: { setVersion: (version: string) => any }) {
	const [versions] = useCommand(bridge.commands.getMinecraftVersions);
	const [filteredVersions, setFilteredVersions] = createSignal<bridge.Version[]>([]);
	const [filters, setFilters] = createSignal<VersionReleaseFilters>({
		old_alpha: false,
		old_beta: false,
		release: true,
		snapshot: false,
	});

	function refresh(filters: VersionReleaseFilters, versions: bridge.Version[]) {
		setFilteredVersions(() => {
			if (Object.values(filters).every(value => value === false))
				return versions;

			return versions.filter((version) => {
				return filters[version.type];
			});
		});
	}

	function toggleFilter(name: string) {
		setFilters((prev) => {
			return {
				...prev,
				[name]: !prev[name as keyof VersionReleaseFilters],
			};
		});
	}

	function setVersion(index: number | undefined) {
		if (index === undefined)
			return;

		const versions = untrack(() => filteredVersions);
		const version = versions()[index];

		if (version)
			props.setVersion(version.id);
	}

	createEffect(() => {
		const list = versions();
		if (list !== undefined)
			refresh(filters(), list);
	});

	onMount(() => {
		setFilteredVersions(versions()!);
	});

	return (
		<div class="flex flex-1 flex-row gap-2">
			<SelectList
				class="max-h-40 min-w-3/5"
				onChange={setVersion}
			>
				<Show when={filteredVersions() !== undefined}>
					<Index each={filteredVersions()}>
						{(version, index) => (
							<SelectList.Row index={index}>{version().id}</SelectList.Row>
						)}
					</Index>
				</Show>
			</SelectList>

			<div class="flex flex-1 flex-col gap-y-2 overflow-hidden border border-gray-05 rounded-lg bg-component-bg p-2">
				<For each={Object.keys(filters())}>
					{name => (
						<Checkbox
							defaultChecked={filters()[name as VersionType]!}
							onChecked={() => toggleFilter(name)}
						>
							{formatVersionRelease(name as VersionType)}
						</Checkbox>
					)}
				</For>
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
			<h3 class="text-left text-md text-fg-secondary font-semibold uppercase">{props.header}</h3>
			{/* <div class="max-h-8"> */}
			{props.children}
			{/* </div> */}
		</div>
	);
}
