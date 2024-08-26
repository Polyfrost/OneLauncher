import { For, type ParentProps, Show, createContext, createEffect, createSignal, useContext } from 'solid-js';
import { A, type Params, Route, useSearchParams } from '@solidjs/router';
import { CalendarIcon, ChevronDownIcon, ClockRewindIcon, Download01Icon, File02Icon, HeartIcon, LinkExternal01Icon } from '@untitled-theme/icons-solid';
import type { Cluster, ManagedPackage, ManagedUser, Providers } from '@onelauncher/client/bindings';
import { getLicenseUrl, getPackageUrl } from '../../../utils';
import { bridge } from '~imports';
import useCommand from '~ui/hooks/useCommand';
import { abbreviateNumber, formatAsRelative } from '~utils';
import Tooltip from '~ui/components/base/Tooltip';
import Markdown from '~ui/components/content/Markdown';
import Button from '~ui/components/base/Button';
import Dropdown from '~ui/components/base/Dropdown';
import useBrowser from '~ui/hooks/useBrowser';
import SteveHead from '~assets/images/steve.png';
import usePromptOpener from '~ui/hooks/usePromptOpener';
import Link from '~ui/components/base/Link';

interface BrowserModParams extends Params {
	id: string;
	provider: Providers;
}

const BrowserPackageContext = createContext<ManagedPackage | null>(null);

const basePath = '/browser/package';

function BrowserPackageProvider(props: ParentProps & { pkg: ManagedPackage }) {
	return (
		// eslint-disable-next-line solid/reactivity -- Should be fine
		<BrowserPackageContext.Provider value={props.pkg}>
			{props.children}
		</BrowserPackageContext.Provider>
	);
}

function BrowserPackageRoutes() {
	return (
		<>
			<Route path="/" component={BrowserPackageBody} />
			<Route path="/gallery" component={BrowserPackageGallery} />
			<Route path="/versions" component={BrowserPackageVersions} />
		</>
	);
}

function NavLink(props: ParentProps & { href: string }) {
	const [params] = useSearchParams<BrowserModParams>();

	const url = () => {
		const searchParams = new URLSearchParams(params as Record<string, string>);
		return `${basePath}${props.href}?${searchParams.toString()}`;
	};

	return (
		<A
			class="rounded-md px-4 py-2 text-sm text-fg-primary font-semibold uppercase active:bg-component-bg-pressed hover:bg-component-bg-hover"
			activeClass="bg-component-bg-pressed hover:bg-component-bg-pressed"
			href={url()}
			children={props.children}
		/>
	);
}

function BrowserPackage(props: ParentProps) {
	const [params] = useSearchParams<BrowserModParams>();
	const [pkg] = useCommand(() => bridge.commands.getProviderPackage(params.provider!, params.id!));
	const [authors] = useCommand(pkg, () => bridge.commands.getProviderAuthors(pkg()!.provider, pkg()!.author));

	const links = [
		['About', '/'],
		['Gallery', '/gallery'],
		['Versions', '/versions'],
	];

	return (
		<>
			<div class="flex flex-row items-start gap-x-4">
				{/* TODO: Make a progress bar of some sort */}
				<Show
					when={pkg() !== undefined && authors() !== undefined}
					fallback={<div>Loading...</div>}
					children={(
						<>
							<BrowserSidebar package={pkg()!} authors={authors()!} />

							<div class="flex flex-1 flex-col items-start justify-between gap-y-4">
								<div class="flex flex-row gap-x-1 rounded-lg bg-component-bg p-1">
									<For each={links}>
										{link => (
											<NavLink href={link[1] || ''}>
												{link[0]}
											</NavLink>
										)}
									</For>
								</div>

								<BrowserPackageProvider pkg={pkg()!}>
									{/* <AnimatedRoutes> */}
									{props.children}
									{/* </AnimatedRoutes> */}
								</BrowserPackageProvider>
							</div>
						</>
					)}
				/>
			</div>
		</>
	);
}

BrowserPackage.buildUrl = function (params: BrowserModParams): string {
	return `${basePath}?id=${params.id}&provider=${params.provider}`;
};

BrowserPackage.Routes = BrowserPackageRoutes;

export default BrowserPackage;

function BrowserSidebar(props: { package: ManagedPackage; authors: ManagedUser[] }) {
	const createdAt = () => new Date(props.package.created);
	const updatedAt = () => new Date(props.package.updated);
	const promptOpen = usePromptOpener();

	return (
		<div class="sticky top-0 max-w-60 min-w-54 flex flex-col gap-y-4">
			<div class="min-h-72 flex flex-col overflow-hidden rounded-lg bg-component-bg">
				<div class="relative h-28 flex items-center justify-center overflow-hidden">
					<img class="absolute z-0 max-w-none w-7/6 filter-blur-xl" src={props.package.icon_url || ''} alt={`Icon for ${props.package.title}`} />
					<img class="relative z-1 aspect-ratio-square w-2/5 rounded-md image-render-auto" src={props.package.icon_url || ''} alt={`Icon for ${props.package.title}`} />
				</div>
				<div class="flex flex-1 flex-col gap-2 p-3">
					<div class="flex flex-col gap-2">
						<h4 class="text-fg-primary font-medium">{props.package.title}</h4>
						<p class="text-xs text-fg-secondary">
							<span class="text-fg-primary capitalize">{props.package.package_type}</span>
							{' '}
							on
							{' '}
							<span class="text-fg-primary">{props.package.provider}</span>
						</p>
					</div>

					<p class="max-h-22 flex-1 overflow-hidden text-sm text-fg-secondary line-height-snug">{props.package.description}</p>

					<div class="flex flex-row gap-4 text-xs">
						<div class="flex flex-row items-center gap-2">
							<Download01Icon class="h-4 w-4" />
							{abbreviateNumber(props.package.downloads)}
						</div>

						<div class="flex flex-row items-center gap-2">
							<HeartIcon class="h-4 w-4" />
							{abbreviateNumber(props.package.followers)}
						</div>
					</div>
				</div>
			</div>

			<InstallButton {...props.package} />

			<div class="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 class="text-fg-primary font-bold">Links</h4>
				<Link includeIcon href={getPackageUrl(props.package.provider, props.package.id, props.package.package_type)}>
					{props.package.provider}
					{' '}
					Page
				</Link>
			</div>

			<div class="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 class="text-fg-primary font-bold">Authors</h4>
				<For each={props.authors}>
					{author => (
						<div
							class="flex flex-row items-center gap-x-1 rounded-md p-1 active:bg-component-bg-pressed hover:bg-component-bg-hover"
							onClick={() => promptOpen(author.url)}
						>
							<img class="h-8 min-h-8 min-w-8 w-8 rounded-md" src={author.avatar_url || SteveHead} alt={`${author.username}'s avatar`} />
							<span class="flex-1">{author.username}</span>
							<LinkExternal01Icon class="h-4 w-4" />
						</div>
					)}
				</For>
			</div>

			<div class="flex flex-col gap-2 rounded-lg bg-component-bg p-3 text-xs!">
				<h4 class="text-fg-primary font-bold">Details</h4>
				<Show when={props.package.license !== null}>
					<div class="flex flex-row items-start gap-x-1">
						<File02Icon class="h-3 min-w-3 w-3" />
						License
						<Link includeIcon href={getLicenseUrl(props.package.license)}>
							{props.package.license?.name || props.package.license?.id || 'Unknown'}
						</Link>
					</div>
				</Show>

				<Tooltip text={createdAt().toLocaleString()}>
					<div class="flex flex-row items-center gap-x-1">
						<CalendarIcon class="h-3 min-w-3 w-3" />
						Created
						<span class="text-fg-primary font-medium">
							{formatAsRelative(createdAt().getTime(), 'en', 'long')}
						</span>
					</div>
				</Tooltip>

				<Tooltip text={updatedAt().toLocaleString()}>
					<div class="flex flex-row items-center gap-x-1">
						<ClockRewindIcon class="h-3 min-w-3 w-3" />
						Last Updated
						<span class="text-fg-primary font-medium">
							{formatAsRelative(updatedAt().getTime(), 'en', 'long')}
						</span>
					</div>
				</Tooltip>
			</div>

		</div>
	);
}

function InstallButton(props: ManagedPackage) {
	const [selected, setSelected] = createSignal(0);
	const [clusters] = useCommand(() => bridge.commands.getClusters());
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
		bridge.commands.downloadProviderPackage(
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
				class="max-w-full flex-1 rounded-r-none!"
				disabled={filtered()?.length === 0}
				onClick={download}
				children={(
					<div class="flex flex-1 flex-col items-center justify-center">
						<p class="text-xs">Download latest to</p>
						<span class="mt-0.5 h-3.5 max-w-38 overflow-x-hidden text-sm font-bold">{filtered?.()?.[selected?.()]?.meta.name || 'Unknown'}</span>
					</div>
				)}
			/>

			<Dropdown
				class="w-8"
				dropdownClass="w-58! right-0"
				disabled={filtered()?.length === 0}
				selected={selected}
				onChange={setSelected}
				component={props => (
					<Button
						buttonStyle="primary"
						iconLeft={<ChevronDownIcon />}
						class="h-full w-full border-l border-white/5 rounded-l-none! px-0!"
						onClick={() => props.setVisible(true)}
					/>
				)}
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

// Sub pages
function BrowserPackageBody() {
	const context = useContext(BrowserPackageContext);

	return (
		<div class="w-full flex-1 rounded-lg bg-component-bg p-4 px-6">
			<Markdown body={context?.body || ''} />
		</div>
	);
}

function BrowserPackageGallery() {
	// const context = useContext(BrowserPackageContext);

	return (
		<div class="w-full flex-1 rounded-lg bg-component-bg p-4 px-6">
			<div>Gallery</div>
			{/* <For each={[]}>
				{image => (
					<img src={image.url} alt={image.alt} />
				)}
			</For> */}
		</div>
	);
}

function BrowserPackageVersions() {
	// const context = useContext(BrowserPackageContext);

	return (
		<div class="w-full flex-1 rounded-lg bg-component-bg p-4 px-6">
			<div>versions</div>
		</div>
	);
}
