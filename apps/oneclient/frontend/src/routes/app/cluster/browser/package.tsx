import type { ManagedPackage, ManagedUser, ManagedVersion, PackageDonationUrl } from '@/bindings.gen';
import { LoaderSuspense } from '@/components';
import { ExternalLink } from '@/components/ExternalLink';
import { Markdown } from '@/components/Markdown';
import { bindings } from '@/main';
import { abbreviateNumber, formatAsRelative, upperFirst, useCommand, useCommandMut, useCommandSuspense } from '@onelauncher/common';
import { Button, Show, Tooltip } from '@onelauncher/common/components';
import { createFileRoute, redirect } from '@tanstack/react-router';
import { CalendarIcon, CheckIcon, ClockRewindIcon, Download01Icon, File02Icon, Loading02Icon, XIcon } from '@untitled-theme/icons-react';
import { useMemo } from 'react';
import { Cell, Column, Row, Tab, Table, TableBody, TableHeader, TabList, TabPanel, Tabs } from 'react-aria-components';

export interface PackageRouteSearchParams {
	packageId: string;
}

export const Route = createFileRoute('/app/cluster/browser/package')({
	component: RouteComponent,
	validateSearch: (search): PackageRouteSearchParams => ({
		packageId: search.packageId as string,
	}),
	async beforeLoad({ search }) {
		if (!search.packageId)
			throw redirect({ to: '/app/cluster', from: '/app/cluster/browser/package', search });

		const managedPackage = await bindings.core.getPackage(search.provider, search.packageId);

		return { managedPackage };
	},
});

function RouteComponent() {
	const { managedPackage } = Route.useRouteContext();

	return (
		<div className="h-full flex flex-1 flex-row items-start gap-x-4">
			<BrowserSidebar package={managedPackage} />

			<Tabs className="min-h-full flex flex-1 flex-col items-start gap-y-4 pb-8">
				<TabList className="flex flex-none flex-row gap-x-1 rounded-lg bg-component-bg p-1 w-min">
					<Tab className="uppercase p-2.5 text-trim rounded-md selected:bg-component-bg-pressed disabled:hidden" id="about">About</Tab>
					<Tab className="uppercase p-2.5 text-trim rounded-md selected:bg-component-bg-pressed disabled:hidden" id="versions">Versions</Tab>
					{/* <Tab className="uppercase p-2.5 text-trim rounded-md selected:bg-component-bg-pressed disabled:hidden" id="gallery" isDisabled={managedPackage.gallery.length === 0}>Gallery</Tab> */}
				</TabList>

				<TabPanel className="flex-1 w-full" id="about">
					<LoaderSuspense>
						<BodyPanel />
					</LoaderSuspense>
				</TabPanel>

				<TabPanel className="flex-1 w-full" id="versions">
					<LoaderSuspense>
						<Versions />
					</LoaderSuspense>
				</TabPanel>

				<TabPanel className="flex flex-wrap gap-2 justify-center" id="gallery">

					{/* {managedPackage.gallery.map(item => (
							<Modal.Trigger key={item.url}>
								<Pressable>
									<div aria-label="Loaders" className="rounded-md overflow-hidden bg-component-bg-pressed outline-0 h-64 flex flex-col relative">
										<img className="h-full" src={item.thumbnail_url} />
										{item.title && <div className="absolute w-full bottom-0 bg-component-bg-disabled/80 p-2">{item.title}</div>}
									</div>
								</Pressable>
								<Modal className="w-full">
									<img className="rounded-xl" src={item.url} />
								</Modal>
							</Modal.Trigger>
						))} */}

				</TabPanel>
			</Tabs>
		</div>
	);
}

function BodyPanel() {
	const { provider } = Route.useSearch();
	const { managedPackage } = Route.useRouteContext();

	const { data: body } = useCommandSuspense(['getPackageBody', provider, managedPackage.body], () => bindings.core.getPackageBody(provider, managedPackage.body));

	return (
		<div className="prose prose-invert prose-sm max-w-none prose-code:before:content-none prose-code:after:content-none prose-code:bg-component-bg-disabled prose-code:rounded-sm prose-code:p-1! prose-code:text-trim prose-img:inline-block prose-a:inline-block">
			<div className="h-full min-h-full flex-1 w-full rounded-lg bg-component-bg p-3">
				<Markdown body={body} />
			</div>
		</div>
	);
}

function getPackageUrl(pkg: ManagedPackage): string {
	switch (pkg.provider) {
		case 'Modrinth': return `https://modrinth.com/project/${pkg.slug}`;
		case 'CurseForge': return `https://www.curseforge.com/minecraft/${pkg.package_type}s/${pkg.slug}`;
		case 'SkyClient': return ``;
	}
}

function BrowserSidebar({ package: pkg }: { package: ManagedPackage }) {
	const { provider } = Route.useSearch();

	const createdAt = useMemo(() => pkg.created ? new Date(pkg.created) : null, [pkg.created]);
	const updatedAt = useMemo(() => pkg.updated ? new Date(pkg.updated) : null, [pkg.updated]);

	const authors = useCommand(['getUsersFromAuthor', pkg.author], () => bindings.core.getUsersFromAuthor(provider, pkg.author));

	return (
		<div className="z-1 max-w-60 min-w-54 flex flex-col gap-y-4 mb-6">
			<div className="flex flex-col overflow-hidden rounded-lg bg-component-bg">
				<div className="relative h-28 flex items-center justify-center overflow-hidden">
					<img alt={`Icon for ${pkg.name}`} className="absolute z-0 max-w-none w-7/6 blur-xl" src={pkg.icon_url || ''} />
					<img alt={`Icon for ${pkg.name}`} className="relative z-1 aspect-ratio-square w-2/5 rounded-md image-render-auto" src={pkg.icon_url || ''} />
				</div>
				<div className="flex flex-1 flex-col gap-2 p-3">
					<div className="flex flex-col gap-2">
						<h4 className="text-fg-primary font-medium line-height-snug">{pkg.name}</h4>
						<p className="text-xs text-fg-secondary">
							<span className="text-fg-primary capitalize">{pkg.package_type}</span>
							{' '}
							on
							{' '}
							<span className="text-fg-primary">{pkg.provider}</span>
						</p>
					</div>

					<p className="max-h-22 flex-1 overflow-hidden text-sm text-fg-secondary line-height-snug">{pkg.short_desc}</p>

					<div className="flex flex-row gap-4 text-xs">
						{pkg.provider !== 'SkyClient' && (
							<div className="flex flex-row items-center gap-2">
								<Download01Icon className="h-4 w-4" />
								{abbreviateNumber(pkg.downloads)}
							</div>
						)}
					</div>
				</div>
			</div>

			<InstallButton />

			<div className="flex flex-col rounded-lg bg-component-bg p-3">
				<h4 className="text-fg-primary font-bold">Links</h4>
				<div className="flex flex-col">
					<ExternalLink className="text-link hover:text-link-hover" href={getPackageUrl(pkg)} includeIcon>
						{provider}
						{' '}
						Page
					</ExternalLink>
					{(Object.entries(pkg.links) as Array<[keyof typeof pkg.links, typeof pkg.links[keyof typeof pkg.links]]>).filter(a => a[1]).map(link =>
						typeof link[1] == 'string'
							? (
									<ExternalLink
										children={upperFirst(link[0])}
										className="text-link hover:text-link-hover"
										href={link[1]}
										includeIcon
										key={link[0]}
									/>
								)
							: (link[1] as Array<PackageDonationUrl>).map(donationLink => (
									<ExternalLink
										children={upperFirst(donationLink.id)}
										className="text-link hover:text-link-hover"
										href={donationLink.url}
										includeIcon
										key={donationLink.id}
									/>
								)))}
				</div>
			</div>

			<div className="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 className="text-fg-primary font-bold">Authors</h4>
				{
					authors.isSuccess
						? authors.data.map(author => <Author author={author} key={author.id} />)
						: <h3>Loading...</h3>
				}

			</div>

			<div className="flex flex-col gap-2 rounded-lg bg-component-bg p-3 text-xs!">
				<h4 className="text-fg-primary font-bold">Details</h4>
				{pkg.license !== null && (
					<div className="flex flex-row items-start gap-x-1">
						<File02Icon className="h-3 min-w-3 w-3" />
						License
						<ExternalLink href={pkg.license.url ?? undefined}>
							{pkg.license.name || pkg.license.id || 'Unknown'}
						</ExternalLink>
					</div>
				)}

				{createdAt !== null && (
					<Tooltip text={createdAt.toLocaleString()}>
						<div className="flex flex-row items-center gap-x-1">
							<CalendarIcon className="h-3 min-w-3 w-3" />
							Created
							<span className="text-fg-primary font-medium">
								{formatAsRelative(createdAt.getTime(), 'en', 'long')}
							</span>
						</div>
					</Tooltip>
				)}

				{updatedAt !== null && (
					<Tooltip text={updatedAt.toLocaleString()}>
						<div className="flex flex-row items-center gap-x-1">
							<ClockRewindIcon className="h-3 min-w-3 w-3" />
							Last Updated
							<span className="text-fg-primary font-medium">
								{formatAsRelative(updatedAt.getTime(), 'en', 'long')}
							</span>
						</div>
					</Tooltip>
				)}
			</div>

		</div>
	);
}

function usePackageVersions() {
	const { provider, packageId } = Route.useSearch();
	const { cluster } = Route.useRouteContext();
	const { data: versions } = useCommandSuspense(
		['getPackageVersions', provider, packageId, cluster.mc_version, cluster.mc_loader],
		() => bindings.core.getPackageVersions(provider, packageId, cluster.mc_version, cluster.mc_loader, 0, 64),
	);

	return versions;
}

function InstallButton() {
	const { cluster, managedPackage } = Route.useRouteContext();
	const packageVersions = usePackageVersions();

	const { provider } = Route.useSearch();
	const download = useCommandMut(() => bindings.core.downloadPackage(provider, managedPackage.id, packageVersions.items[0].version_id, cluster.id, true));
	return (
		<Button
			className="flex-col py-4"
			isDisabled={packageVersions.total === 0 || !download.isIdle}
			onClick={() => download.mutate()}
		>
			<p className="text-lg">{download.isIdle
				? 'Download'
				: download.isPending
					? 'Installing'
					: 'Installed'}
			</p>

			<p className="text-xs">
				to
				{' '}
				<strong>{cluster.name}</strong>
			</p>
		</Button>
	);
}

function Author({ author }: { author: ManagedUser }) {
	return (
		<ExternalLink
			className="no-underline flex flex-row items-center gap-x-1 rounded-md p-1 active:bg-component-bg-pressed hover:bg-component-bg-hover"
			href={author.url ?? undefined}
		>
			<img alt={`${author.username}'s avatar`} className="h-8 min-h-8 min-w-8 w-8 rounded-[5px]" src={author.avatar_url || ''} />
			<div className="flex flex-1 flex-col justify-center gap-y-px">
				<span>{author.username}</span>

				{author.is_organization_user && (
					<span className="text-xs text-fg-secondary">Organization</span>
				)}

				{author.role !== null && (
					<span className="text-xs text-fg-secondary">{author.role}</span>
				)}
			</div>
		</ExternalLink>

	);
}

function colorForType(type: string) {
	switch (type.toLowerCase()) {
		case 'release':
			return 'bg-code-trace/30';
		case 'snapshot':
			return 'bg-code-debug/30';
		case 'beta':
			return 'bg-code-warn/30';
		case 'alpha':
			return 'bg-code-error/30';
		default:
			return 'bg-border/30';
	}
}

function Versions() {
	// const { provider } = Route.useSearch();
	// const [offset, setOffset] = useState(0);

	// const { data: versions } = usePackageVersions(provider, id, {
	// 	limit: 20,
	// 	offset,
	// });

	const versions = usePackageVersions();

	// const pagination = usePagination({
	// 	itemsCount: versions?.total as unknown as number,
	// 	itemsPerPage: versions?.limit as unknown as number,
	// });
	// const paginationRef = useRef(pagination);
	// useEffect(() => {
	// 	setOffset(pagination.offset);
	// }, [pagination.offset]);
	// useEffect(() => {
	// 	paginationRef.current.reset();
	// }, [versions?.total]);
	return (
		<div className="flex flex-col w-full">
			<div className="flex justify-between">
				{/* <pagination.Navigation /> */}
			</div>
			<Table className="border-separate border-spacing-x-none border-spacing-y-1 flex-1 w-full">
				<TableHeader className="text-left">
					<Column className="pr-2" />
					<Column className="pr-4" isRowHeader>Name</Column>
					<Column className="pr-4">Game Versions</Column>
					<Column className="pr-4">Loaders</Column>
					<Column className="pr-4">Created</Column>
					<Column className="pr-4">Downloads</Column>
					<Column className="pr-4" />
				</TableHeader>
				<TableBody className="w-full">
					{versions.items.map(item =>
						<VersionRow key={item.version_id} version={item} />)}
				</TableBody>
			</Table>
		</div>
	);
}
function VersionRow({ version }: { version: ManagedVersion }) {
	const { cluster } = Route.useRouteContext();
	const { provider } = Route.useSearch();
	const download = useCommandMut(() => bindings.core.downloadPackage(provider, version.project_id, version.version_id, cluster.id, true));
	return (
		<Row className="my-2 bg-page-elevated px-4 [&>td]:py-4 w-full">
			<Cell className="p-4 my-2 bg-component-bg rounded-l-xl">
				<Tooltip text={upperFirst(version.release_type)}>
					<div className={`${colorForType(version.release_type)} h-8 w-8 flex items-center justify-center rounded-md`}>
						<span className="font-bold">{version.release_type.charAt(0).toUpperCase()}</span>
					</div>
				</Tooltip>
			</Cell>
			<Cell className="p-4 px-2 bg-component-bg">{version.display_name}</Cell>
			<Cell className="p-4 pl-0 bg-component-bg">{version.mc_versions.join(', ')}</Cell>
			<Cell className="p-4 pl-0 bg-component-bg">{version.loaders.map(upperFirst).join(', ')}</Cell>
			<Cell className="p-4 pl-0 bg-component-bg">{formatAsRelative(Date.parse(version.published))}</Cell>
			<Cell className="p-4 pl-0 bg-component-bg">{version.downloads}</Cell>
			<Cell className="p-4 pl-0 bg-component-bg rounded-r-xl">
				<Button
					// children={download.isSuccess ? <CheckIcon /> : }
					color="secondary"
					isDisabled={!download.isIdle}
					onClick={() => download.mutate()}
					size="icon"
				>
					<Show children={<Download01Icon />} when={download.isIdle} />
					<Show children={<CheckIcon />} when={download.isSuccess} />
					<Show children={<Loading02Icon className="animate-spin animate-duration-2000" />} when={download.isPending} />
					<Show children={<XIcon />} when={download.error} />
				</Button>

			</Cell>
		</Row>
	);
}
