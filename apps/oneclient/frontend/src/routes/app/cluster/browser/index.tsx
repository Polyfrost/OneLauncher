import type { PackageType, Provider } from '@/bindings.gen';
import { browserCategories, categoryNameFromId } from '@/utils/browser';
import { Dropdown, Show, TextField } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { SearchMdIcon } from '@untitled-theme/icons-react';
import { useMemo } from 'react';

interface BrowserSearchRouteSearchParams {
	query?: string;
	packageType: PackageType;
	categories: Array<string>;
}

export const Route = createFileRoute('/app/cluster/browser/')({
	component: RouteComponent,
	validateSearch: (search): BrowserSearchRouteSearchParams => {
		return {
			query: search.query as string,
			packageType: search.packageType as PackageType,
			categories: search.categories as Array<string>,
		};
	},
});

const PROVIDERS: Array<Provider> = ['Modrinth', 'CurseForge', 'SkyClient'];

function RouteComponent() {
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

						</div>
					</div>

					{/* <BrowserSidebar /> */}
				</div>
			</div>
		</div>
	);
}

function BrowserCategories() {
	const search = Route.useSearch();
	const { categories, packageType } = search;
	const navigate = Route.useNavigate();
	const switchCategory = useMemo(() => (category: string) => {
		const newCategories = categories.includes(category)
			? categories.filter(element => element !== category)
			: [...categories, category];
		navigate({
			search: {
				...search,
				categories: newCategories,
			},
		});
	}, []);

	return (
		<div className="top-0 h-fit min-w-50">
			{/* <div /> */}
			<div className="flex flex-col gap-y-6 ml-16">
				<Show when>
					<div className="flex flex-col gap-y-1.5">
						<h5 className="my-1 text-sm uppercase opacity-60 font-light">Categories</h5>
						{browserCategories[categoryNameFromId[packageType]].map(category => (
							<p
								aria-selected={categories.includes(category)}
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
	const search = Route.useSearch();
	const { provider, query } = search;
	const navigate = Route.useNavigate();
	const setQuery = (query: string) => {
		navigate({ search: { ...search, query } });
	};
	return (
		<div className="w-full flex flex-row justify-between bg-page">
			<div className="flex flex-row gap-2">
				<Dropdown
					onSelectionChange={id => navigate({ search: { ...search, provider: id as Provider } })}
					placeholder="Select a Provider"
					selectedKey={provider}
				>
					{PROVIDERS.map(provider => (
						<Dropdown.Item id={provider} key={provider}>
							<div className="flex flex-row">
								{/* <ProviderIcon className="size-4 mr-2 self-center" provider={provider} /> */}
								{provider}
							</div>
						</Dropdown.Item>
					))}
				</Dropdown>
			</div>

			<div className="flex flex-row gap-2">
				<TextField
					className="min-w-64"
					iconLeft={<SearchMdIcon className="scale-75" />}
					// onChange={e => setQuery(e.currentTarget.value)}
					onKeyDown={(e) => {
						if (e.key !== 'Enter')
							return;
						e.preventDefault();
						setQuery(e.currentTarget.value);
					}}
					placeholder="Search for content"
					value={query}
				/>
			</div>
		</div>
	);
}
