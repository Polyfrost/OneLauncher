import type { Filters, PackageCategories } from '@/bindings.gen';
import type { Key, NonReadonly } from '@/utils';
import type { PropsWithChildren } from 'react';
import ProviderIcon from '@/components/content/ProviderIcon';
import { BrowserProvider, useBrowserContext } from '@/hooks/useBrowser';
import { useClusters } from '@/hooks/useCluster';
import { bindings } from '@/main';
import { PROVIDERS, upperFirst } from '@/utils';
import { browserCategories, categoryNameFromId } from '@/utils/browser';
import { useCommand } from '@onelauncher/common';
import { AnimatedOutlet, Dropdown, Show, TextField } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { SearchMdIcon } from '@untitled-theme/icons-react';
import { useEffect, useMemo, useState } from 'react';
import { SelectValue } from 'react-aria-components';

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
			<div className="h-full w-full flex flex-1 flex-col items-center gap-y-2">
				<div className="grid grid-cols-[180px_auto_160px] w-full gap-x-6">
					<div />
					<BrowserToolbar />
					<div />
				</div>

				<div className="grid grid-cols-[180px_auto_160px] w-full gap-x-6 pb-8">
					<BrowserCategories />

					<div className="h-full flex flex-col gap-y-4">
						<div className="h-full flex-1">
							{children}
						</div>
					</div>

					{/* <BrowserSidebar /> */}
				</div>
			</div>
		</div>
	);
}

const defaultFilters: Filters = {
	categories: null,
	game_versions: null,
	loaders: null,
	package_type: null,
};

type CategoryFilter = Partial<NonReadonly<typeof browserCategories>>;

function BrowserCategories() {
	const context = useBrowserContext();
	const categoryName = useMemo(() => categoryNameFromId[context.query.filters?.package_type ?? 'mod'], [context.query.filters]);
	const categories = browserCategories[categoryName];

	const switchCategory = useMemo(() => (category: string) => {
		const isCorrectPackageType = context.query.filters && context.query.filters.categories && categoryName in context.query.filters.categories;
		const isEnabled = isCorrectPackageType && context.query.filters.categories[categoryName].includes(category);
		const newCategories = isCorrectPackageType
			? { [categoryName]: isEnabled ? context.query.filters.categories?.[categoryName].filter(c => c !== category) : [...context.query.filters.categories[categoryName], category] }
			: { [categoryName]: [category] };

		context.setQuery({ ...context.query, filters: { ...defaultFilters, ...context.query.filters, categories: newCategories } });
	}, [context, categoryName]);

	return (
		<div className="top-0 h-fit min-w-50">
			{/* <div /> */}
			<div className="flex flex-col gap-y-6 ml-16">
				<Show when>
					<div className="flex flex-col gap-y-1.5">
						<h5 className="my-1 text-sm uppercase opacity-60 font-light">Categories</h5>
						{categories.map(category => (
							<p
								aria-selected={context.query.filters?.categories?.[categoryName].includes(category)}
								className="text-sm capitalize opacity-60 hover:opacity-90 text-fg-primary hover:text-fg-primary-hover aria-selected:opacity-100"
								key={category}
								onClick={() => switchCategory(category)}
							>
								{category}
							</p>
						))}
					</div>
				</Show>
			</div>
		</div>
	);
}

function BrowserToolbar() {
	const clusters = useClusters();
	const context = useBrowserContext();
	const [query, setQuery] = useState(context.query.query ?? '');
	useEffect(() => {
		const game_versions = context.cluster?.mc_version ? [context.cluster.mc_version] : null;
		const loaders = context.cluster?.mc_loader ? [context.cluster.mc_loader] : null;
		context.setQuery({ ...context.query, filters: { ...defaultFilters, ...context.query.filters, game_versions, loaders } });
	// eslint-disable-next-line react-hooks/exhaustive-deps -- loop
	}, [context.cluster]);
	return (
		<div className="w-full flex flex-row justify-between bg-page">
			<div className="flex flex-row gap-2">
				<Dropdown
					onSelectionChange={id => context.setProvider(id as typeof context.provider)}
					placeholder="Select a Provider"
					selectedKey={context.provider}
				>
					{PROVIDERS.map(provider => (
						<Dropdown.Item id={provider} key={provider}>
							<div className="flex flex-row">
								<ProviderIcon className="size-4 mr-2 self-center" provider={provider} />
								{provider}
							</div>
						</Dropdown.Item>
					))}
				</Dropdown>
				<Dropdown
					onSelectionChange={(index) => {
						if (!clusters)
							return;
						const cluster = clusters[index as number];
						context.setCluster(cluster);
					}}
					placeholder="Select a cluster"
					selectedKey={context.cluster ? clusters?.indexOf(context.cluster) : undefined}
				>
					{clusters?.map((cluster, index) => (
						<Dropdown.Item id={index} key={cluster.id}>
							{cluster.name}
						</Dropdown.Item>
					))}
				</Dropdown>
			</div>

			<div className="flex flex-row gap-2">
				<TextField
					className="min-w-64"
					iconLeft={<SearchMdIcon className="scale-75" />}
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
