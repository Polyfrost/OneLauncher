import type { Provider, SearchResult } from '@/bindings.gen';
import OneConfigLogo from '@/assets/logos/oneconfig.svg';
import { useBrowserContext, useBrowserSearch } from '@/hooks/useBrowser';
import { abbreviateNumber } from '@/utils';
import { Button, Show } from '@onelauncher/common/components';
import { createFileRoute, useSearch } from '@tanstack/react-router';
import { ChevronRightIcon, Download01Icon, HeartIcon } from '@untitled-theme/icons-react';
import { useEffect } from 'react';
import { BrowserLayout } from './route';

export const Route = createFileRoute('/app/browser/')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<BrowserLayout>
			<div className="flex flex-col gap-8">
				<Featured />
				<Search />
			</div>
		</BrowserLayout>
	);
}

function Featured() {
	const context = useBrowserContext();
	return (
		<Show when={!context.query.query && !(context.query.filters?.categories && context.query.filters.categories.length > 0)}>
			<div className="flex flex-col gap-y-1">
				<h5 className="ml-2">Featured</h5>
				<div className="w-full flex flex-row overflow-hidden rounded-lg bg-page-elevated">
					<div className="w-full p-1">
						<img alt="thumbnail" className="aspect-ratio-video h-full rounded-md object-cover object-center" src="https://cdn.modrinth.com/data/AANobbMI/295862f4724dc3f78df3447ad6072b2dcd3ef0c9_96.webp" />
					</div>
					<div className="max-w-84 min-w-52 flex flex-col gap-y-1 p-4">
						<h2>Sodium</h2>

						<Show when={false}>
							<div className="w-fit flex flex-row items-center gap-x-1 rounded-lg bg-border/10 px-1.5 py-1 text-fg-primary transition hover:opacity-80">
								<img alt="OneConfig Logo" className="h-3.5 w-3.5" src={OneConfigLogo} />
								<span className="text-sm font-medium">OneConfig Integrated</span>
							</div>
						</Show>

						<p className="mt-1 flex-1 leading-normal">Peak mod</p>

						<div className="flex flex-row justify-end">
							<Button color="ghost">
								View Package
								<ChevronRightIcon />
							</Button>
						</div>
					</div>
				</div>
			</div>
		</Show>
	);
}

function Search() {
	const context = useBrowserContext();
	const search = useBrowserSearch(context.provider, context.query);
	useEffect(() => {
		search.refetch();
	}, [context.provider, context.query]);

	return (
		<div className="grid grid-cols-2 xl:grid-cols-3 gap-2">
			<Show when={search.isSuccess}>
				{search.data?.items.map(item => (
					<PackageItem key={item.project_id} {...item} provider={context.provider} />
				))}
			</Show>
		</div>
	);
}

function PackageItem(props: SearchResult & { provider: Provider }) {
	function redirect() {
	}

	return (
		<div
			className="h-full min-w-50 flex overflow-hidden rounded-lg bg-component-bg hover:bg-component-bg-hover flex-col max-h-74 min-h-74"
			onClick={redirect}
			tabIndex={0}
		>
			<div
				className="relative flex items-center justify-center overflow-hidden w-full h-28"
			>
				<Show
					fallback={(
						<div
							className="aspect-ratio-square rounded-md bg-border/05 w-2/5"
						/>
					)}
					when={props.icon_url}
				>
					<img alt={`Icon for ${props.title}`} className="absolute z-0 max-w-none w-7/6 opacity-50 filter-blur-xl" src={props.icon_url} />
					<img
						alt={`Icon for ${props.title}`}
						className="relative z-1 aspect-ratio-square rounded-md image-render-auto w-2/5"
						src={props.icon_url}
					/>
				</Show>
			</div>
			<div className="flex flex-1 flex-col gap-2 p-3">
				<div className="flex flex-col gap-2">
					<h4 className="text-fg-primary font-medium line-height-normal">{props.title}</h4>
					<p className="text-xs text-fg-secondary">
						By
						{' '}
						<span className="text-fg-primary">{props.author}</span>
						{' '}
						on
						{' '}
						<span className="text-fg-primary">{props.provider}</span>
					</p>
				</div>

				<p className="max-h-22 flex-1 overflow-hidden text-sm text-fg-secondary line-height-snug">{props.description}</p>

				<div className="flex flex-row gap-4 text-xs">
					<Show when={props.provider !== 'SkyClient'}>
						<div className="flex flex-row items-center gap-2">
							<Download01Icon className="h-4 w-4" />
							{abbreviateNumber(props.downloads)}
						</div>

						{/* <Show when={props.follows > 0}>
							<div className="flex flex-row items-center gap-2">
								<HeartIcon className="h-4 w-4" />
								{abbreviateNumber(props.follows)}
							</div>
						</Show> */}
					</Show>
				</div>
			</div>
		</div>
	);
}
