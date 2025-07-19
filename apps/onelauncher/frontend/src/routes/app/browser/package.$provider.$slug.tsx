import type { ClusterModel, ManagedPackage, ManagedUser, ManagedVersion, Paginated, Provider } from '@/bindings.gen'
import { useBrowserContext, usePackageData, usePackageVersions } from '@/hooks/useBrowser'
import { useClusters } from '@/hooks/useCluster'
import { bindings } from '@/main'
import { abbreviateNumber, formatAsRelative, PROVIDERS } from '@/utils'
import { useCommand } from '@onelauncher/common'
import { Show, Tooltip, Button, Dropdown } from '@onelauncher/common/components'
import { createFileRoute, Link } from '@tanstack/react-router'
import { openUrl } from '@tauri-apps/plugin-opener'
import { Download01Icon, LinkExternal01Icon, File02Icon, CalendarIcon, ClockRewindIcon, ChevronDownIcon, ChevronUpIcon } from '@untitled-theme/icons-react'
import { createContext, useContext, useEffect, useMemo, useRef, useState } from 'react'
import { Collection, ListBox, ListBoxItem, Popover, Select } from 'react-aria-components'


export const Route = createFileRoute('/app/browser/package/$provider/$slug')({
  component: RouteComponent,
})

function includes<T, C extends T>(list:{includes: (arg0:C)=>boolean}, element:T): element is C{
	return list.includes(element as unknown as C)
}

type PackageContextType = {
	pkg: ManagedPackage|undefined
	versions: Paginated<ManagedVersion>|undefined
}

const PackageContext = createContext<PackageContextType>({
	pkg: undefined,
	versions: undefined
})

function RouteComponent() {
	const {provider, slug} = Route.useParams()
	if(!includes(PROVIDERS, provider)) throw new Error("Invalid provider");
	const packageData = usePackageData(provider, slug)
	const browserContext = useBrowserContext()
	const {data: versions} = usePackageVersions(provider, slug, {
				mc_versions: browserContext.cluster ? [browserContext.cluster.mc_version] : null,
				loaders: browserContext.cluster ? [browserContext.cluster.mc_loader] : null,
				limit: 50})

	return (
		<PackageContext value={{pkg: packageData.data, versions}}>
			<Show when={packageData.isSuccess && !packageData.isFetching}>
				<div className="h-full flex flex-1 flex-row items-start gap-x-4">
					<BrowserSidebar package={packageData.data!} />

					<div className="min-h-full flex flex-1 flex-col items-start gap-y-4 pb-8">
						<div className="flex flex-none flex-row gap-x-1 rounded-lg bg-component-bg p-1">
							<Link to='/app/browser/package/$provider/$slug' params={{provider, slug}}>About</Link>
							<Link to='/app/browser/package/$provider/$slug' params={{provider, slug}}>Versions</Link>


						</div>

						<div className="h-full min-h-full w-full flex-1">

						</div>
					</div>
				</div>
			</Show>
		</PackageContext>
	)
}


function BrowserSidebar({package: pkg}: { package: ManagedPackage }) {
	const {provider} = Route.useParams()

	const createdAt = useMemo(() => pkg.created ? new Date(pkg.created) : null, []);
	const updatedAt = useMemo(() => pkg.updated ? new Date(pkg.updated) : null, []);

	const authors = useCommand("getUsersFromAuthor", ()=>bindings.core.getUsersFromAuthor(provider as Provider, pkg.author))

	return (
		<div className="sticky top-0 z-1 max-w-60 min-w-54 flex flex-col gap-y-4">
			<div className="min-h-72 flex flex-col overflow-hidden rounded-lg bg-component-bg">
				<div className="relative h-28 flex items-center justify-center overflow-hidden">
					<img alt={`Icon for ${pkg.name}`} className="absolute z-0 max-w-none w-7/6 filter-blur-xl" src={pkg.icon_url || ''} />
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

			<InstallButton {...pkg} />

			{/* <div className="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 className="text-fg-primary font-bold">Links</h4>
				<Link href={getPackageUrl(contentPackage)} includeIcon>
					{contentPackage.provider}
					{' '}
					Page
				</Link>
			</div> */}

			<div className="flex flex-col gap-2 rounded-lg bg-component-bg p-3">
				<h4 className="text-fg-primary font-bold">Authors</h4>
				{
					authors.isSuccess
					? authors.data.map(author=><Author author={author}/>)
					: <h3>Loading...</h3>
				}

			</div>

			<div className="flex flex-col gap-2 rounded-lg bg-component-bg p-3 text-xs!">
				<h4 className="text-fg-primary font-bold">Details</h4>
				<Show when={pkg.license !== null}>
					<div className="flex flex-row items-start gap-x-1">
						<File02Icon className="h-3 min-w-3 w-3" />
						License
						<Link to={pkg.license?.url ?? "#"}>
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
			return 'bg-border/05';
	}
}

function InstallButton({...pkg}:ManagedPackage){
	const triggerRef = useRef<HTMLButtonElement>(null)
	const elementRef = useRef<HTMLDivElement>(null)
	const {provider, slug} = Route.useParams()
	const [open, setOpen] = useState(false)
	const clusters = useClusters()
	const browserContext = useBrowserContext()
	const {versions} = useContext(PackageContext)
	const version = useMemo(()=>{
		if(!versions || !browserContext.cluster) return undefined;
		return versions.items.findLast(version=>
			version.mc_versions.includes(browserContext.cluster!.mc_version)
			&& version.loaders.includes(browserContext.cluster!.mc_loader)
		)
	},[browserContext.cluster, versions])

	function download(){
		if(!version || !browserContext.cluster || !includes(PROVIDERS, provider)) return false;
		downloadPackage(browserContext.cluster, provider, version)
	}


	return (
	<Select onOpenChange={setOpen} isOpen={open}
	aria-label='cluster'
	onSelectionChange={e=>{
		const cluster = clusters?.find(cluster=>cluster.id as unknown as number == e)
		if(cluster) browserContext.setCluster(cluster)
	}}>
		<div className="h-12 flex flex-row w-full" ref={elementRef}>
			<Button
				color={version ? "primary" : "secondary"}
				className="max-w-full flex-1 rounded-r-none!"
				onClick={download}
				isDisabled={!version}
			>
				<Download01Icon/>
				<div className='w-full text-sm'>
					{
						browserContext.cluster ?
							version
								? <span>Download to <br /><span className='text-md font-semibold'>{browserContext.cluster.name}</span></span>
								: `No matching version found`
							: "Select a Cluster"
					}
				</div>
			</Button>
			<Button className="w-8 rounded-l-none border-l border-white/10" onClick={()=>setOpen(!open)} ref={triggerRef}>
				{open
					? <ChevronUpIcon/>
					: <ChevronDownIcon/>
				}
			</Button>
		</div>
		<Popover triggerRef={triggerRef} className="mt-1 rounded-lg shadow-md bg-component-bg border border-component-border" style={{width: `${elementRef.current?.clientWidth}px`}}>
			<ListBox className="outline-none flex flex-col gap-0.5">
				<Collection items={clusters}>
					{item=> <ListBoxItem id={item.id as unknown as number}
					className="group/item flex flex-row items-center justify-between gap-2 rounded-lg p-2 w-full">{item.name}</ListBoxItem>}
				</Collection>
			</ListBox>
		</Popover>
	</Select>

	)
}


function Author({author}:{author:ManagedUser}){

	//TODO: onclick and fallback avatar url
	return (
	<a
		className="flex flex-row items-center gap-x-1 rounded-md p-1 active:bg-component-bg-pressed hover:bg-component-bg-hover"
		onClick={()=>{if(author.url) openUrl(author.url)}}
	>
		<img alt={`${author.username}'s avatar`} className="h-8 min-h-8 min-w-8 w-8 rounded-[5px]" src={author.avatar_url || ""} />
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

	)
}

function downloadPackage(cluster: ClusterModel, provider:Provider, version: ManagedVersion, skipCompatibility = false){
	return bindings.core.downloadPackage(provider, version.project_id, version.version_id, cluster.id, skipCompatibility)
}
