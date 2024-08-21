import { For, Show, createEffect, createSignal } from 'solid-js';
import { type Params, useSearchParams } from '@solidjs/router';
import { ChevronDownIcon, Download01Icon, HeartIcon } from '@untitled-theme/icons-solid';
import type { Cluster, ManagedPackage, Providers } from '@onelauncher/client/bindings';
import { getLicenseUrl, getPackageUrl } from '../../../utils';
import { bridge } from '~imports';
import useCommand from '~ui/hooks/useCommand';
import { abbreviateNumber, formatAsRelative } from '~utils';
import Tooltip from '~ui/components/base/Tooltip';
import Markdown from '~ui/components/content/Markdown';
import Link from '~ui/components/base/Link';
import Button from '~ui/components/base/Button';
import Dropdown from '~ui/components/base/Dropdown';
import useBrowser from '~ui/hooks/useBrowser';

interface BrowserModParams extends Params {
	id: string;
	provider: Providers;
}

function BrowserPackage() {
	const [params] = useSearchParams<BrowserModParams>();
	const [pkg] = useCommand(bridge.commands.getPackage, params.provider!, params.id!);

	return (
		<>
			<div class="flex flex-row gap-x-4">
				{/* TODO: Make a progress bar of some sort */}
				<Show
					when={pkg() !== undefined}
					fallback={<div>Loading...</div>}
					children={(
						<>
							<BrowserSidebar {...pkg()!} />

							<div class="flex flex-1 flex-col items-start justify-between gap-y-4 rounded-lg bg-component-bg p-4 px-6">
								<div class="flex-1">
									<Markdown body={pkg()!.body} />
								</div>
							</div>
						</>
					)}
				/>
			</div>
		</>
	);
}

BrowserPackage.buildUrl = function (params: BrowserModParams): string {
	return `/browser/package?id=${params.id}&provider=${params.provider}`;
};

export default BrowserPackage;

function BrowserSidebar(props: ManagedPackage) {
	const createdAt = () => new Date(props.created);
	const updatedAt = () => new Date(props.updated);

	return (
		<div class="max-w-60 min-w-54 flex flex-col gap-y-4">
			<div class="min-h-72 flex flex-col overflow-hidden rounded-lg bg-component-bg">
				<div class="relative h-28 flex items-center justify-center overflow-hidden">
					<img class="absolute z-0 max-w-none w-7/6 filter-blur-xl" src={props.icon_url || ''} alt={`Icon for ${props.title}`} />
					<img class="relative z-1 aspect-ratio-square w-2/5 rounded-md image-render-auto" src={props.icon_url || ''} alt={`Icon for ${props.title}`} />
				</div>
				<div class="flex flex-1 flex-col gap-2 p-3">
					<div class="flex flex-col gap-2">
						<h4 class="text-fg-primary font-medium">{props.title}</h4>
						<p class="text-xs text-fg-secondary">
							<span class="text-fg-primary capitalize">{props.package_type}</span>
							{' '}
							on
							{' '}
							<span class="text-fg-primary">{props.provider}</span>
						</p>
					</div>

					<p class="max-h-22 flex-1 overflow-hidden text-sm text-fg-secondary line-height-snug">{props.description}</p>

					<div class="flex flex-row gap-4 text-xs">
						<div class="flex flex-row items-center gap-2">
							<Download01Icon class="h-4 w-4" />
							{abbreviateNumber(props.downloads)}
						</div>

						<div class="flex flex-row items-center gap-2">
							<HeartIcon class="h-4 w-4" />
							{abbreviateNumber(props.followers)}
						</div>
					</div>
				</div>
			</div>

			<InstallButton {...props} />

			<div class="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 class="text-fg-primary font-medium">Links</h4>
				<Link includeIcon href={getPackageUrl(props.provider, props.id, props.package_type)}>
					{props.provider}
					{' '}
					Page
				</Link>
			</div>

			<div class="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 class="text-fg-primary font-medium">Authors</h4>
			</div>

			<div class="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 class="text-fg-primary font-medium">Details</h4>
				<Show when={props.license !== null}>
					<div class="flex flex-row items-start gap-x-1">
						License

						<Link includeIcon href={getLicenseUrl(props.license)}>
							{props.license?.name || props.license?.id || 'Unknown'}
						</Link>
					</div>
				</Show>

				<Tooltip text={createdAt().toLocaleString()}>
					Created
					{' '}
					<span class="text-fg-primary font-medium">
						{formatAsRelative(createdAt().getTime(), 'en', 'long')}
					</span>
				</Tooltip>

				<Tooltip text={updatedAt().toLocaleString()}>
					Last Updated
					{' '}
					<span class="text-fg-primary font-medium">
						{formatAsRelative(updatedAt().getTime(), 'en', 'long')}
					</span>
				</Tooltip>
			</div>

		</div>
	);
}

function InstallButton(props: ManagedPackage) {
	const [selected, setSelected] = createSignal(0);
	const [clusters] = useCommand(bridge.commands.getClusters);
	const controller = useBrowser();

	const meetsRequirements = (cluster: Cluster) => {
		const game_version = props.game_versions.includes(cluster.meta.mc_version);
		const loader = props.loaders.includes(cluster.meta.loader || 'vanilla');

		return game_version && loader;
	};

	const filtered = () => clusters()?.filter(meetsRequirements);

	const getSelectedCluster = () => filtered()?.[selected()];

	function download() {
		const cluster = getSelectedCluster();

		if (cluster === undefined)
			return;

		// TODO: Add a progress bar
		bridge.commands.downloadPackage(
			props.provider,
			props.id,
			cluster.uuid,
			cluster.meta.mc_version,
			cluster.meta.loader || null,
			null, // TODO: Specific package version
		);
	}

	createEffect(() => {
		if (filtered() === undefined)
			return;

		const cluster = controller.cluster();

		if (cluster !== undefined && meetsRequirements(cluster)) {
			const index = filtered()!.findIndex(c => c.uuid === cluster.uuid);
			if (index !== -1)
				setSelected(index);
		}
	});

	return (
		<div class="h-12 flex flex-row">

			<Button
				buttonStyle="primary"
				iconLeft={<Download01Icon />}
				children={(
					<div class="flex flex-1 flex-col items-center justify-center">
						<p class="text-xs">Download to</p>
						<span class="mt-0.5 h-3.5 max-w-38 overflow-x-hidden text-sm font-bold">{filtered?.()?.[selected?.()]?.meta.name || 'Unknown'}</span>
					</div>
				)}
				class="max-w-full flex-1 rounded-r-none!"

				onClick={download}
			/>

			<Dropdown
				component={props => (
					<Button
						buttonStyle="primary"
						iconLeft={<ChevronDownIcon />}
						class="h-full w-full border-l border-white/5 rounded-l-none! px-0!"
						onClick={() => props.setVisible(true)}
					/>
				)}
				class="w-8"
				dropdownClass="w-58! right-0"
				selected={selected}
				onChange={setSelected}
			>
				<For each={filtered()}>
					{cluster => (
						<Dropdown.Row>{cluster.meta.name}</Dropdown.Row>
					)}
				</For>
			</Dropdown>
		</div>
	);
}
