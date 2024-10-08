import type { Cluster, ManagedPackage, ManagedVersion, Providers } from '@onelauncher/client/bindings';
import { A, type Params, Route, useSearchParams } from '@solidjs/router';
import { CalendarIcon, ChevronDownIcon, ClockRewindIcon, Download01Icon, File02Icon, HeartIcon, LinkExternal01Icon } from '@untitled-theme/icons-solid';
import SteveHead from '~assets/images/steve.png';
import { bridge } from '~imports';
import AnimatedRoutes from '~ui/components/AnimatedRoutes';
import Button from '~ui/components/base/Button';
import Dropdown from '~ui/components/base/Dropdown';
import Link from '~ui/components/base/Link';
import Tooltip from '~ui/components/base/Tooltip';
import Markdown from '~ui/components/content/Markdown';
import { createModal } from '~ui/components/overlay/Modal';
import Spinner from '~ui/components/Spinner';
import useBrowser from '~ui/hooks/useBrowser';
import { ChooseClusterModal } from '~ui/hooks/useCluster';
import useCommand, { tryResult } from '~ui/hooks/useCommand';
import usePagination from '~ui/hooks/usePagination';
import usePromptOpener from '~ui/hooks/usePromptOpener';
import { abbreviateNumber, formatAsRelative } from '~utils';
import { type Accessor, createContext, createEffect, createMemo, createResource, createSignal, For, Match, on, type ParentProps, type Resource, Show, Switch, useContext } from 'solid-js';
import { getLicenseUrl, getPackageUrl, upperFirst } from '../../../utils';

interface BrowserModParams extends Params {
	id: string;
	provider: Providers;
}

interface BrowserPackageContextType {
	pkg: Accessor<ManagedPackage>;
	body: Resource<string>;
};

const BrowserPackageContext = createContext<BrowserPackageContextType | null>(null);

const basePath = '/browser/package';

function BrowserPackageProvider(props: ParentProps & { pkg: ManagedPackage }) {
	const [body] = useCommand(() => bridge.commands.getPackageBody(props.pkg.provider, props.pkg.body));

	const ctx: BrowserPackageContextType = {
		pkg: () => props.pkg,
		body,
	};

	return (
		<BrowserPackageContext.Provider value={ctx}>
			{props.children}
		</BrowserPackageContext.Provider>
	);
}

function BrowserPackageRoutes() {
	return (
		<>
			<Route component={BrowserPackageBody} path="/" />
			{/* <Route path="/gallery" component={BrowserPackageGallery} /> */}
			<Route component={BrowserPackageVersions} path="/versions" />
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
			activeClass="bg-component-bg-pressed hover:bg-component-bg-pressed"
			children={props.children}
			class="rounded-md px-4 py-2 text-sm text-fg-primary font-semibold uppercase active:bg-component-bg-pressed hover:bg-component-bg-hover"
			href={url()}
		/>
	);
}

function BrowserPackage(props: ParentProps) {
	const [params] = useSearchParams<BrowserModParams>();
	const [pkg] = useCommand(() => bridge.commands.getProviderPackage(params.provider!, params.id!));

	const links = [
		['About', '/'],
		// ['Gallery', '/gallery'],
		['Versions', '/versions'],
	];

	return (
		<Spinner.Suspense>
			<div class="h-full flex flex-1 flex-row items-start gap-x-4">
				<Show
					children={(
						<>
							<BrowserSidebar package={pkg()!} />

							<div class="min-h-full flex flex-1 flex-col items-start gap-y-4 pb-8">
								<div class="flex flex-none flex-row gap-x-1 rounded-lg bg-component-bg p-1">
									<For each={links}>
										{link => (
											<NavLink href={link[1] || ''}>
												{link[0]}
											</NavLink>
										)}
									</For>
								</div>

								<div class="h-full min-h-full w-full flex-1">
									<BrowserPackageProvider pkg={pkg()!}>
										<Spinner.Suspense>
											<div class="h-full max-w-full min-h-full w-full overflow-hidden">
												<AnimatedRoutes>
													{props.children}
												</AnimatedRoutes>
											</div>
										</Spinner.Suspense>
									</BrowserPackageProvider>
								</div>
							</div>
						</>
					)}
					when={pkg() !== undefined}
				/>
			</div>
		</Spinner.Suspense>
	);
}

BrowserPackage.buildUrl = function (params: BrowserModParams): string {
	return `${basePath}/?id=${params.id}&provider=${params.provider}`;
};

BrowserPackage.Routes = BrowserPackageRoutes;

export default BrowserPackage;

function BrowserSidebar(props: { package: ManagedPackage }) {
	const [authors] = useCommand(() => props.package, () => bridge.commands.getProviderAuthors(props.package.provider, props.package.author));
	const createdAt = () => new Date(props.package.created);
	const updatedAt = () => new Date(props.package.updated);
	const promptOpen = usePromptOpener();

	return (
		<div class="sticky top-0 z-1 max-w-60 min-w-54 flex flex-col gap-y-4">
			<div class="min-h-72 flex flex-col overflow-hidden rounded-lg bg-component-bg">
				<div class="relative h-28 flex items-center justify-center overflow-hidden">
					<img alt={`Icon for ${props.package.title}`} class="absolute z-0 max-w-none w-7/6 filter-blur-xl" src={props.package.icon_url || ''} />
					<img alt={`Icon for ${props.package.title}`} class="relative z-1 aspect-ratio-square w-2/5 rounded-md image-render-auto" src={props.package.icon_url || ''} />
				</div>
				<div class="flex flex-1 flex-col gap-2 p-3">
					<div class="flex flex-col gap-2">
						<h4 class="text-fg-primary font-medium line-height-snug">{props.package.title}</h4>
						<p class="text-xs text-fg-secondary">
							<span class="text-fg-primary capitalize">{props.package.package_type}</span>
							{' '}
							on
							{' '}
							<span class="text-fg-primary">{props.package.provider}</span>
						</p>
						<Show when={props.package.is_archived === true}>
							<p class="w-fit rounded-full bg-code-warn/10 px-2 py-1 text-xs text-code-warn">Archived</p>
						</Show>
					</div>

					<p class="max-h-22 flex-1 overflow-hidden text-sm text-fg-secondary line-height-snug">{props.package.description}</p>

					<div class="flex flex-row gap-4 text-xs">
						<div class="flex flex-row items-center gap-2">
							<Download01Icon class="h-4 w-4" />
							{abbreviateNumber(props.package.downloads)}
						</div>

						<Show when={props.package.followers > 0}>
							<div class="flex flex-row items-center gap-2">
								<HeartIcon class="h-4 w-4" />
								{abbreviateNumber(props.package.followers)}
							</div>
						</Show>
					</div>
				</div>
			</div>

			<InstallButton {...props.package} />

			<div class="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 class="text-fg-primary font-bold">Links</h4>
				<Link href={getPackageUrl(props.package)} includeIcon>
					{props.package.provider}
					{' '}
					Page
				</Link>
			</div>

			<div class="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 class="text-fg-primary font-bold">Authors</h4>
				<For each={authors()}>
					{author => (
						<>
							<div
								class="flex flex-row items-center gap-x-1 rounded-md p-1 active:bg-component-bg-pressed hover:bg-component-bg-hover"
								onClick={() => promptOpen(author.url)}
							>
								<img alt={`${author.username}'s avatar`} class="h-8 min-h-8 min-w-8 w-8 rounded-[5px]" src={author.avatar_url || SteveHead} />
								<div class="flex flex-1 flex-col justify-center gap-y-1">
									<span>{author.username}</span>

									<Show when={author.is_organization_user}>
										<span class="text-xs text-fg-secondary">Organization</span>
									</Show>

									<Show when={author.role !== null}>
										<span class="text-xs text-fg-secondary">{author.role}</span>
									</Show>
								</div>
								<LinkExternal01Icon class="h-4 w-4" />
							</div>
							<Show when={author.is_organization_user === true && ((authors()?.length || 0) > 1)}>
								<div class="h-px w-full bg-gray-05" />
							</Show>
						</>
					)}
				</For>
			</div>

			<div class="flex flex-col gap-2 rounded-lg bg-component-bg p-3 text-xs!">
				<h4 class="text-fg-primary font-bold">Details</h4>
				<Show when={props.package.license !== null}>
					<div class="flex flex-row items-start gap-x-1">
						<File02Icon class="h-3 min-w-3 w-3" />
						License
						<Link href={getLicenseUrl(props.package.license)} includeIcon>
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

	const filtered = createMemo(() => clusters()?.filter(meetsRequirements));

	const getSelectedCluster = () => filtered()?.[selected()];

	async function download() {
		const cluster = getSelectedCluster();

		if (cluster === undefined)
			return;

		// TODO: Add a progress bar
		try {
			await bridge.commands.downloadProviderPackage(
				props.provider,
				props.id,
				cluster.uuid,
				cluster.meta.mc_version,
				cluster.meta.loader || null,
				null, // TODO: Specific package version
			);

			await bridge.commands.syncClusterPackages(cluster.path || '');
		}
		catch (err) {
			console.error(err);
		}
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
		<Show when={(filtered()?.length || 0) > 0}>
			<div class="h-12 flex flex-row">

				<Button
					buttonStyle="primary"
					children={(
						<div class="flex flex-1 flex-col items-center justify-center">
							<p class="text-xs">Download latest to</p>
							<span class="mt-0.5 h-3.5 max-w-38 overflow-x-hidden text-sm font-bold">{filtered?.()?.[selected?.()]?.meta.name || 'Unknown'}</span>
						</div>
					)}
					class="max-w-full flex-1 rounded-r-none!"
					iconLeft={<Download01Icon />}
					onClick={download}
				/>

				<Dropdown
					class="w-8"
					component={props => (
						<Button
							buttonStyle="primary"
							class="h-full w-full border-l border-white/5 rounded-l-none! px-0!"
							iconLeft={<ChevronDownIcon />}
							onClick={() => props.setVisible(true)}
						/>
					)}
					dropdownClass="w-58! right-0"
					onChange={setSelected}
					selected={selected}
				>
					<For each={filtered()}>
						{cluster => (
							<Dropdown.Row>{cluster.meta.name}</Dropdown.Row>
						)}
					</For>
				</Dropdown>
			</div>
		</Show>
	);
}

// Sub pages
function BrowserPackageBody() {
	const context = useContext(BrowserPackageContext);

	return (
		<div class="w-full flex-1 rounded-lg bg-component-bg p-4 px-6">
			<Markdown body={context?.body?.() || ''} />
		</div>
	);
}

// function BrowserPackageGallery() {
// 	// const context = useContext(BrowserPackageContext);

// 	return (
// 		<div class="w-full flex-1 rounded-lg bg-component-bg p-4 px-6">
// 			<div>Gallery</div>
// 			{/* <For each={[]}>
// 				{image => (
// 					<img src={image.url} alt={image.alt} />
// 				)}
// 			</For> */}
// 		</div>
// 	);
// }

function BrowserPackageVersions() {
	const context = useContext(BrowserPackageContext);
	const MAX_ITEMS_PER_PAGE = 20;

	const { page, Navigation, reset, options } = usePagination({
		itemsCount: () => context?.pkg().versions.length || 0,
		itemsPerPage: () => MAX_ITEMS_PER_PAGE,
	});

	const getVersionsForPage = (page: number) => {
		if (context === null)
			return [];

		const newestToOldest = context.pkg().versions.toReversed();
		const start = (page - 1) * MAX_ITEMS_PER_PAGE;
		const end = start + MAX_ITEMS_PER_PAGE;

		return newestToOldest.slice(start, end);
	};

	const [versions, { refetch }] = createResource(context, async () => {
		if (context === null)
			throw new Error('Context is null');

		if (context.pkg().versions.length === 0) {
			const [data, pagination] = (await tryResult(() => bridge.commands.getAllProviderPackageVersions(context.pkg().provider, context.pkg().id, null, null, page() - 1, MAX_ITEMS_PER_PAGE))) || [];

			if (options().itemsCount() !== pagination.totalCount)
				reset({
					itemsCount: () => pagination.totalCount || 0,
					itemsPerPage: () => MAX_ITEMS_PER_PAGE,
				});

			return data;
		}

		const list = getVersionsForPage(page());

		return (await tryResult(() => bridge.commands.getProviderPackageVersions(context.pkg().provider, list))).toReversed();
	});

	let container!: HTMLDivElement;

	createEffect(on(() => page(), () => {
		refetch();
		container.parentElement?.parentElement?.scrollIntoView({ behavior: 'smooth' });
	}));

	return (
		<div class="h-full w-full flex flex-1 flex-col gap-y-1" ref={container}>
			<div class="flex flex-row justify-between">
				<h1>
					Versions - Page
					{' '}
					{page()}
				</h1>

				<Navigation />
			</div>

			<Spinner.Suspense>
				<table class="w-full border-separate border-spacing-x-none border-spacing-y-1">
					<thead>
						<tr class="bg-page-elevated [&>th]:py-2 [&>th]:text-left">
							<th class="w-16 rounded-l-lg" />

							<th>Name</th>
							<th>Game Version</th>
							<th>Loader</th>
							<th>Created</th>
							<th>Downloads</th>
							<th class="rounded-r-lg" />
						</tr>
					</thead>

					<tbody>
						<For each={versions() || []}>
							{version => <VersionRow {...version} />}
						</For>
					</tbody>
				</table>
			</Spinner.Suspense>

			<Navigation />

		</div>
	);
}

function colorForType(type: string) {
	switch (type.toLowerCase()) {
		case 'release':
			return 'bg-code-trace';
		case 'snapshot':
			return 'bg-code-debug';
		case 'beta':
			return 'bg-code-warn';
		case 'alpha':
			return 'bg-code-error';
		default:
			return 'bg-gray-05';
	}
}

function VersionRow(props: ManagedVersion) {
	const [clusters, setClusters] = createSignal<Cluster[]>([]);
	const context = useContext(BrowserPackageContext);

	async function getClusters() {
		const clusters = await tryResult(bridge.commands.getClusters);

		return clusters.filter((cluster) => {
			const game_version = props.game_versions.includes(cluster.meta.mc_version);
			const loader = props.loaders.includes(cluster.meta.loader || 'vanilla');

			return game_version && loader;
		});
	}

	const modal = createModal((props) => {
		return (
			<ChooseClusterModal
				clusters={clusters}
				onSelected={downloadPackage}
				{...props}
			/>
		);
	});

	async function downloadPrompted() {
		const clusterz = await getClusters();
		setClusters(clusterz);

		modal.show();
	}

	async function downloadPackage(cluster: Cluster) {
		if (cluster === undefined)
			return;

		try {
			// eslint-disable-next-line solid/reactivity -- lint issue
			await tryResult(() => bridge.commands.downloadProviderPackage(
				context!.pkg()!.provider,
				context!.pkg()!.id,
				cluster.uuid,
				cluster.meta.mc_version,
				cluster.meta.loader || null,
				props.id,
			));

			await bridge.commands.syncClusterPackages(cluster.path || '');
		}
		catch (err) {
			console.error(err);
		}
	}

	return (
		<tr class="my-2 bg-page-elevated px-4 [&>td]:py-4">
			<td class="rounded-l-lg px-4">
				<Tooltip text={upperFirst(props.version_type)}>
					<div class={`${colorForType(props.version_type)} h-8 w-8 flex items-center justify-center rounded-md bg-opacity-30`}>
						<span class="font-bold">{props.version_type.charAt(0).toUpperCase()}</span>
					</div>
				</Tooltip>
			</td>

			<td>
				<div class="flex flex-col gap-2">
					<h3 class="text-lg">{props.version_display}</h3>
					<p class="text-wrap text-sm">{props.name === props.version_display ? props.files[0]?.file_name : props.name}</p>
				</div>
			</td>

			<td>
				<Switch>
					<Match when={props.game_versions.length > 3}>
						<Tooltip text={props.game_versions.join(', ')}>
							<span>
								{props.game_versions.slice(-3).toReversed().join(', ')}
								...
							</span>
						</Tooltip>
					</Match>
					<Match when>
						<span>{props.game_versions.join(', ')}</span>
					</Match>
				</Switch>
			</td>

			<td>
				{props.loaders.map(upperFirst).join(', ')}
			</td>

			<td>
				{formatAsRelative(new Date(props.published).getTime(), 'en', 'long')}
			</td>

			<td>
				{props.downloads.toLocaleString()}
			</td>

			<td class="rounded-r-lg">
				<Button
					buttonStyle="iconSecondary"
					children={<Download01Icon />}
					onClick={downloadPrompted}
				/>
			</td>

		</tr>
	);
}
