import type { Filters } from '@/bindings.gen';
import type { PropsWithChildren } from 'react';
import ProviderIcon from '@/components/content/ProviderIcon';
import { BrowserProvider, useBrowserContext } from '@/hooks/useBrowser';
import { useClusters } from '@/hooks/useCluster';
import { PROVIDERS } from '@/utils';
import { browserCategories } from '@/utils/browser';
import { AnimatedOutlet, Dropdown, Show, TextField } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { SearchMdIcon } from '@untitled-theme/icons-react';
import { useEffect, useState } from 'react';

export const Route = createFileRoute('/app/browser')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<BrowserProvider>
			<div>
				<AnimatedOutlet
					enter={{ animate: { opacity: 1 }, initial: { opacity: 1 } }}
					exit={{ animate: { opacity: 0 }, initial: { opacity: 0 } }}
					from={Route.id}
				/>
			</div>
		</BrowserProvider>
	);
}

export function BrowserLayout({ children }: PropsWithChildren) {
	return (
		<div className="relative h-full flex flex-1 flex-col items-center gap-2">
			<div className="h-full w-full max-w-screen-2xl flex flex-1 flex-col items-center gap-y-2">
				<div className="grid grid-cols-[220px_auto_220px] w-full gap-x-6">
					<div />
					<BrowserToolbar />
					<div />
				</div>

				<div className="grid grid-cols-[220px_auto_220px] w-full gap-x-6 pb-8">
					<BrowserCategories />

					<div className="h-full flex flex-col gap-y-4">
						<div className="h-full flex-1">
							{children}
						</div>
					</div>

					<BrowserSidebar />
				</div>
			</div>
		</div>
	);
}

function BrowserSidebar() {
	const context = useBrowserContext();
	const clusters = useClusters();

	useEffect(() => {
		const game_versions = context.cluster?.mc_version ? [context.cluster.mc_version] : null;
		const loaders = context.cluster?.mc_loader ? [context.cluster.mc_loader] : null;
		context.setQuery({ ...context.query, filters: { ...defaultFilters, ...context.query.filters, game_versions, loaders } });
	// eslint-disable-next-line react-hooks/exhaustive-deps -- loop
	}, [context.cluster]);

	return (
		<div className="flex flex-col gap-y-4">
			<div className="flex flex-col gap-y-4">
				<div className="flex flex-col gap-y-1">
					<h6 className="my-1 uppercase  opacity-60">Active Cluster</h6>
					<Dropdown
						onSelectionChange={(id) => {
							const cluster = clusters?.find(item => item.id.toString() === id);
							context.setCluster(cluster);
						}}
						selectedKey={context.cluster?.id.toString()}
					>
						{clusters?.map(cluster => (
							<Dropdown.Item id={cluster.id.toString()} key={cluster.id}>
								{cluster.name}
							</Dropdown.Item>
						))}
					</Dropdown>
				</div>
				<div className="flex flex-col gap-y-1">
					<h6 className="my-1 uppercase opacity-60">Provider</h6>
					<Dropdown onSelectionChange={id => context.setProvider(id as typeof context.provider)} selectedKey={context.provider}>
						{PROVIDERS.map(provider => (
							<Dropdown.Item id={provider} key={provider}>
								<div className="flex flex-row">
									<ProviderIcon className="size-4 mr-2 self-center" provider={provider} />
									{provider}
								</div>
							</Dropdown.Item>
						))}
					</Dropdown>
				</div>
			</div>
		</div>
	);
}

const defaultFilters: Filters = {
	categories: null,
	game_versions: null,
	loaders: null,
	package_types: null,
};

function BrowserCategories() {
	const context = useBrowserContext();
	const categories = browserCategories.byPackageType((context.query.filters?.package_types ?? ['mod'])[0], context.provider);

	function switchCategory(category: string) {
		const newCategories = context.query.filters?.categories?.includes(category)
			? context.query.filters.categories.filter(cat => cat !== category)
			: [...(context.query.filters?.categories ?? []), category];
		context.setQuery({ ...context.query, filters: { ...defaultFilters, ...context.query.filters, categories: newCategories.length > 0 ? newCategories : null } });
	}

	return (
		<div className="top-0 grid grid-cols-[1fr_auto] h-fit min-w-50 gap-y-6">
			<div />
			<div className="flex flex-col gap-y-6">
				<Show when>
					<div className="flex flex-col gap-y-2">
						<h5 className="my-1 uppercase opacity-60">Categories</h5>
						{categories.map(category => (
							<p
								aria-selected={context.query.filters?.categories?.includes(category.id)}
								className="text-md capitalize opacity-60 hover:opacity-90 text-fg-primary hover:text-fg-primary-hover aria-selected:opacity-100"
								key={category.id}
								onClick={() => switchCategory(category.id)}
							>
								{category.display}
							</p>
						))}
					</div>
				</Show>
			</div>
		</div>
	);
}

function BrowserToolbar() {
	const context = useBrowserContext();
	const [query, setQuery] = useState(context.query.query ?? '');
	return (
		<div className="w-full flex flex-row justify-between bg-page">
			<div className="flex flex-row gap-2" />

			<div className="flex flex-row gap-2">
				<TextField
					iconLeft={<SearchMdIcon />}
					onChange={e => setQuery(e.currentTarget.value)}
					onKeyDown={(e) => {
						if (e.key !== 'Enter')
							return;
						e.preventDefault();
						context.setQuery({ ...context.query, query: e.currentTarget.value });
					}}
					placeholder="Search for content"
					value={query}
				/>
			</div>
		</div>
	);
}
