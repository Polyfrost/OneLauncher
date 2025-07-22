import type { ClusterModel, ManagedPackage, ManagedUser, ManagedVersion, PackageDonationUrl, Provider } from '@/bindings.gen';
import type { HTMLProps } from 'react';
import Modal from '@/components/overlay/Modal';
import { useBrowserContext, usePackageData, usePackageVersions } from '@/hooks/useBrowser';
import { ChooseClusterModal, useClusters } from '@/hooks/useCluster';
import usePagination from '@/hooks/usePagination';
import { bindings } from '@/main';
import { abbreviateNumber, formatAsRelative, PROVIDERS, upperFirst } from '@/utils';
import { useCommand } from '@onelauncher/common';
import { Button, Show, Tooltip } from '@onelauncher/common/components';
import { createFileRoute, Link } from '@tanstack/react-router';
import { openUrl } from '@tauri-apps/plugin-opener';
import { CalendarIcon, ChevronDownIcon, ChevronUpIcon, ClockRewindIcon, Download01Icon, File02Icon, LinkExternal01Icon } from '@untitled-theme/icons-react';
import { createContext, useEffect, useMemo, useRef, useState } from 'react';
import { Cell, Collection, Column, ListBox, ListBoxItem, Popover, Pressable, Row, Select, Tab, Table, TableBody, TableHeader, TabList, TabPanel, Tabs } from 'react-aria-components';
import Markdown from 'react-markdown';
import rehypeRaw from 'rehype-raw';
import remarkGfm from 'remark-gfm';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/app/browser/package/$provider/$slug')({
	component: RouteComponent,
});

function includes<T, TArray extends T>(list: { includes: (arg0: TArray) => boolean }, element: T): element is TArray {
	return list.includes(element as unknown as TArray);
}

function CustomA({ href, children, includeIcon, className, ...rest }: { href: string; children: any; includeIcon?: boolean } & HTMLProps<HTMLAnchorElement>) {
	return (
		<a
			className={twMerge('text-fg-primary underline', className)}
			href={href}
			onClick={(e) => {
				e.preventDefault();
				openUrl(href);
			}}
			{...rest}
		>
			{children}
			{includeIcon && <LinkExternal01Icon className="inline w-4 ml-1" />}
		</a>
	);
}

interface PackageContextType {
	pkg: ManagedPackage | undefined;
}

const PackageContext = createContext<PackageContextType>({
	pkg: undefined,
});

function RouteComponent() {
	const { provider, slug } = Route.useParams();
	if (!includes(PROVIDERS, provider))
		throw new Error('Invalid provider');
	const packageData = usePackageData(provider, slug, {});
	// const browserContext = useBrowserContext();

	const packageContextValue = useMemo(() => ({
		pkg: packageData.data,
	}), [packageData.data]);

	return (
		<PackageContext value={packageContextValue}>
			<Show when={packageData.isSuccess && !packageData.isFetching}>
				<div className="h-full flex flex-1 flex-row items-start gap-x-4">
					<BrowserSidebar package={packageData.data!} />

					<Tabs className="min-h-full flex flex-1 flex-col items-start gap-y-4 pb-8">
						<TabList className="flex flex-none flex-row gap-x-1 rounded-lg bg-component-bg p-1 w-min">
							<Tab className="uppercase p-2.5 text-trim rounded-md selected:bg-component-bg-pressed disabled:hidden" id="about">About</Tab>
							<Tab className="uppercase p-2.5 text-trim rounded-md selected:bg-component-bg-pressed disabled:hidden" id="versions">Versions</Tab>
							<Tab className="uppercase p-2.5 text-trim rounded-md selected:bg-component-bg-pressed disabled:hidden" id="gallery" isDisabled={packageData.data?.gallery.length === 0}>Gallery</Tab>
						</TabList>
						<TabPanel className="prose prose-invert prose-sm max-w-none prose-code:before:content-none prose-code:after:content-none prose-code:bg-component-bg-disabled prose-code:rounded-sm prose-code:p-1! prose-code:text-trim prose-img:inline-block prose-a:inline-block" id="about">
							<div className="h-full min-h-full flex-1 w-full rounded-lg bg-component-bg p-3">
								<Markdown
									components={{
										a: ({ node, children, ...props }) => <CustomA children={children} href={props.href as string} />,
									}}
									rehypePlugins={[rehypeRaw]}
									remarkPlugins={[remarkGfm]}
								>
									{packageData.data?.body}
								</Markdown>
							</div>
						</TabPanel>
						<TabPanel id="versions">
							<Versions />
						</TabPanel>
						<TabPanel className="flex flex-wrap gap-2 justify-center" id="gallery">

							{packageData.data?.gallery.map(item => (
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
							))}

						</TabPanel>
					</Tabs>

				</div>
			</Show>
		</PackageContext>
	);
}

function getPackageUrl(pkg: ManagedPackage): string {
	switch (pkg.provider) {
		case 'Modrinth': return `https://modrinth.com/project/${pkg.slug}`;
		case 'Curseforge': return `https://www.curseforge.com/minecraft/${pkg.package_type}s/${pkg.slug}`;
		case 'SkyClient': return ``;
	}
}

function BrowserSidebar({ package: pkg }: { package: ManagedPackage }) {
	const { provider } = Route.useParams();

	const createdAt = useMemo(() => pkg.created ? new Date(pkg.created) : null, [pkg.created]);
	const updatedAt = useMemo(() => pkg.updated ? new Date(pkg.updated) : null, [pkg.updated]);

	const authors = useCommand('getUsersFromAuthor', () => bindings.core.getUsersFromAuthor(provider as Provider, pkg.author));

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
						<Show when={pkg.provider !== 'SkyClient'}>
							<div className="flex flex-row items-center gap-2">
								<Download01Icon className="h-4 w-4" />
								{abbreviateNumber(pkg.downloads)}
							</div>
						</Show>
					</div>
				</div>
			</div>

			<InstallButton />

			<div className="flex flex-col rounded-lg bg-component-bg p-3">
				<h4 className="text-fg-primary font-bold">Links</h4>
				<div className="flex flex-col">
					<CustomA className="text-link hover:text-link-hover" href={getPackageUrl(pkg)} includeIcon>
						{provider}
						{' '}
						Page
					</CustomA>
					{(Object.entries(pkg.links) as Array<[keyof typeof pkg.links, typeof pkg.links[keyof typeof pkg.links]]>).filter(a => a[1]).map(link =>
						typeof link[1] == 'string'
							? (
									<CustomA
										children={upperFirst(link[0])}
										className="text-link hover:text-link-hover"
										href={link[1]}
										includeIcon
										key={link[1]}
									/>
								)
							: (link[1] as Array<PackageDonationUrl>).map(donationLink => (
									<CustomA
										children={upperFirst(donationLink.id)}
										className="text-link hover:text-link-hover"
										href={donationLink.url}
										includeIcon
										key={donationLink.url}
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
				<Show when={pkg.license !== null}>
					<div className="flex flex-row items-start gap-x-1">
						<File02Icon className="h-3 min-w-3 w-3" />
						License
						<Link to={pkg.license?.url ?? '#'}>
							{pkg.license?.name || pkg.license?.id || 'Unknown'}
						</Link>
					</div>
				</Show>

				<Show when={createdAt !== null}>
					<Tooltip text={createdAt!.toLocaleString()}>
						<div className="flex flex-row items-center gap-x-1">
							<CalendarIcon className="h-3 min-w-3 w-3" />
							Created
							<span className="text-fg-primary font-medium">
								{formatAsRelative(createdAt!.getTime(), 'en', 'long')}
							</span>
						</div>
					</Tooltip>
				</Show>

				<Show when={updatedAt !== null}>
					<Tooltip text={updatedAt!.toLocaleString()}>
						<div className="flex flex-row items-center gap-x-1">
							<ClockRewindIcon className="h-3 min-w-3 w-3" />
							Last Updated
							<span className="text-fg-primary font-medium">
								{formatAsRelative(updatedAt!.getTime(), 'en', 'long')}
							</span>
						</div>
					</Tooltip>
				</Show>
			</div>

		</div>
	);
}

function InstallButton() {
	const triggerRef = useRef<HTMLDivElement>(null);
	const { provider, slug } = Route.useParams();
	if (!includes(PROVIDERS, provider))
		throw new Error('invalid provider');
	const [open, setOpen] = useState(false);
	const clusters = useClusters();
	const browserContext = useBrowserContext();
	const { data: versions } = usePackageVersions(provider, slug, {
		mc_versions: browserContext.cluster ? [browserContext.cluster.mc_version] : [],
		loaders: browserContext.cluster ? [browserContext.cluster.mc_loader] : [],
		limit: 1,
	});
	const version = useMemo(() => {
		if (!versions || !browserContext.cluster)
			return undefined;
		return versions.items[0];
	}, [browserContext.cluster, versions]);

	function download() {
		if (!version || !browserContext.cluster || !includes(PROVIDERS, provider))
			return false;
		downloadPackage(browserContext.cluster, provider, version);
	}

	return (

		<div className="h-12 flex flex-row w-full relative">
			<Button
				className="max-w-full flex-1 rounded-r-none disabled:text-white/50 disabled:bg-blue-900"
				color="primary"
				isDisabled={!version}
				onClick={download}
			>
				<Download01Icon />
				<div className="w-full text-sm">
					{
						browserContext.cluster
							? version
								? (
										<span>
											Download latest to
											<br />
											<span className="text-md font-semibold">{browserContext.cluster.name}</span>
										</span>
									)
								: `No matching version found`
							: 'Select a Cluster'
					}
				</div>
			</Button>

			<Button className="w-8 h-full rounded-l-none border-l border-white/10 px-2" onClick={() => setOpen(true)}>
				{open
					? <ChevronUpIcon />
					: <ChevronDownIcon />}
			</Button>
			<Select
				aria-label="cluster" isOpen={open}
				onOpenChange={setOpen}
				onSelectionChange={(e) => {
					const cluster = clusters?.find(item => item.id as unknown as number === e);
					if (cluster)
						browserContext.setCluster(cluster);
				}}
			>
				<div className="w-full h-full absolute -z-10" ref={triggerRef}></div>

				<Popover className="mt-1 rounded-lg shadow-md bg-component-bg border border-component-border -translate-x-full" style={{ width: `${triggerRef.current?.clientWidth}px` }} triggerRef={triggerRef}>
					<ListBox className="outline-none flex flex-col gap-0.5">
						<Collection items={clusters}>
							{item => (
								<ListBoxItem
									className="group/item flex flex-row items-center justify-between gap-2 rounded-lg p-2 w-full text-dropdown-item-text hover:bg-component-bg-hover data-[focused]:bg-component-bg-pressed"
									id={item.id as unknown as number}
								>
									{item.name}
								</ListBoxItem>
							)}
						</Collection>
					</ListBox>
				</Popover>
			</Select>
		</div>

	);
}

function Author({ author }: { author: ManagedUser }) {
	// TODO: onclick and fallback avatar url
	return (
		<a
			className="flex flex-row items-center gap-x-1 rounded-md p-1 active:bg-component-bg-pressed hover:bg-component-bg-hover"
			onClick={() => {
				if (author.url)
					openUrl(author.url);
			}}
		>
			<img alt={`${author.username}'s avatar`} className="h-8 min-h-8 min-w-8 w-8 rounded-[5px]" src={author.avatar_url || ''} />
			<div className="flex flex-1 flex-col justify-center gap-y-1">
				<span>{author.username}</span>

				<Show when={author.is_organization_user}>
					<span className="text-xs text-fg-secondary">Organization</span>
				</Show>

				<Show when={author.role !== null}>
					<span className="text-xs text-fg-secondary">{author.role}</span>
				</Show>
			</div>
			<LinkExternal01Icon className="h-4 w-4" />
		</a>

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
	const { provider, slug } = Route.useParams();

	if (!includes(PROVIDERS, provider))
		throw new Error('invalid provider');

	const [offset, setOffset] = useState(0);

	const { data: versions } = usePackageVersions(provider, slug, {
		limit: 20,
		offset,
	});

	const pagination = usePagination({
		itemsCount: versions?.total as unknown as number,
		itemsPerPage: versions?.limit as unknown as number,
	});

	useEffect(() => {
		setOffset(pagination.offset);
	}, [pagination.offset]);

	useEffect(() => {
		pagination.reset();
	}, [pagination, versions?.total]);

	return (
		<div className="flex flex-col">
			<div className="flex justify-between">
				<pagination.Navigation />
			</div>
			<Table className="border-separate border-spacing-x-none border-spacing-y-1">
				<TableHeader className="text-left">
					<Column className="pr-2" />
					<Column className="pr-4" isRowHeader>Name</Column>
					<Column className="pr-4">Game Versions</Column>
					<Column className="pr-4">Loaders</Column>
					<Column className="pr-4">Created</Column>
					<Column className="pr-4">Downloads</Column>
					<Column className="pr-4" />
				</TableHeader>
				<TableBody className="">
					{versions?.items.map(item =>
						<VersionRow key={item.version_id} version={item} />)}
				</TableBody>
			</Table>
		</div>
	);
}

function VersionRow({ version }: { version: ManagedVersion }) {
	const { provider } = Route.useParams();
	if (!includes(PROVIDERS, provider))
		throw new Error('invalid provider');
	return (
		<Row className="my-2 bg-page-elevated px-4 [&>td]:py-4">
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
				<Modal.Trigger>
					<Button
						children={<Download01Icon />}
						color="secondary"
						size="icon"
					/>
					<ChooseClusterModal confirmText="Download" onSelected={cluster => downloadPackage(cluster, provider, version, true)} />
				</Modal.Trigger>
			</Cell>
		</Row>
	);
}

function downloadPackage(cluster: ClusterModel, provider: Provider, version: ManagedVersion, skipCompatibility = false) {
	return bindings.core.downloadPackage(provider, version.project_id, version.version_id, cluster.id, skipCompatibility);
}
