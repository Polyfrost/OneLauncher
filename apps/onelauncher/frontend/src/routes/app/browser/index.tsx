import type { Provider } from '@/bindings.gen';
import OneConfigLogo from '@/assets/logos/oneconfig.svg';
import { PackageGrid } from '@/components/content/PackageItem';
import { useBrowserSearch, usePackageData } from '@/hooks/useBrowser';
import { PROVIDERS } from '@/utils';
import { Button, Show } from '@onelauncher/common/components';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { ChevronRightIcon } from '@untitled-theme/icons-react';
import { useMemo, useState } from 'react';
import { BrowserLayout } from './route';

export const Route = createFileRoute('/app/browser/')({
	component: RouteComponent,

});

function RouteComponent() {
	return (
		<BrowserLayout>
			<div className="flex flex-col gap-8">
				<Featured />
				<Lists />
			</div>
		</BrowserLayout>
	);
}

function Featured() {
	// const context = useBrowserContext();
	const [provider, slug] = useMemo<[Provider, string]>(() => ['Modrinth', 'sodium'], []);
	const featuredPackage = usePackageData(provider, slug, {}, 'getFeatured');
	const navigate = useNavigate();

	return (
		<Show when={featuredPackage.isSuccess}>
			<div className="flex flex-col gap-y-1">
				<h5 className="ml-2">Featured</h5>
				<div className="w-full flex flex-row overflow-hidden rounded-lg bg-page-elevated">
					<div className="w-full p-1">
						<img alt="thumbnail" className="aspect-ratio-video h-full rounded-md object-cover object-center" src={featuredPackage.data?.icon_url ?? ''} />
					</div>
					<div className="max-w-84 min-w-52 flex flex-col gap-y-1 p-4">
						<h2>{featuredPackage.data?.name}</h2>

						<Show when={false}>
							<div className="w-fit flex flex-row items-center gap-x-1 rounded-lg bg-border/10 px-1.5 py-1 text-fg-primary transition hover:opacity-80">
								<img alt="OneConfig Logo" className="h-3.5 w-3.5" src={OneConfigLogo} />
								<span className="text-sm font-medium">OneConfig Integrated</span>
							</div>
						</Show>

						<p className="mt-1 flex-1 leading-normal">{featuredPackage.data?.short_desc}</p>

						<div className="flex flex-row justify-end">
							<Button color="ghost" onClick={() => navigate({ to: '/app/browser/package/$provider/$slug', params: { provider, slug } })}>
								View
								<ChevronRightIcon />
							</Button>
						</div>
					</div>
				</div>
			</div>
		</Show>
	);
}

function Lists() {
	return (
		<div>
			{PROVIDERS.map(provider => (
				<List key={provider} provider={provider} />
			))}
		</div>
	);
}

function List({ provider }: { provider: Provider }) {
	const search = useBrowserSearch(provider, {
		filters: null,
		limit: 18 as unknown as bigint,
		offset: null,
		query: null,
		sort: null,
	});
	const [expanded, setExpanded] = useState(false);
	return (
		<div>
			<h5 className="uppercase p-3 opacity-60">{provider}</h5>
			<div className={`relative overflow-hidden ${expanded ? '' : 'h-128'}`}>
				{search.isSuccess
					? <PackageGrid items={search.data.items} provider={provider} />
					: <h3>Loading...</h3>}
				<Show when={!expanded}>
					<div className="absolute w-full bottom-0 flex justify-center p-10 left-0 right-0 z-10 bg-gradient-to-b from-transparent to-page to-20%">
						<Button color="secondary" onClick={() => setExpanded(true)}>Show More</Button>
					</div>
				</Show>
			</div>
		</div>
	);
}
