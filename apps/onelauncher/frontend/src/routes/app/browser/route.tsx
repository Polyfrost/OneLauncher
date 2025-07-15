import ProviderIcon from '@/components/content/ProviderIcon';
import { PROVIDERS } from '@/utils';
import { browserCategories } from '@/utils/browser';
import { Button, Dropdown, Show, TextField } from '@onelauncher/common/components';
import { createFileRoute, Outlet } from '@tanstack/react-router';
import { SearchMdIcon } from '@untitled-theme/icons-react';
import { memo } from 'react';

export const Route = createFileRoute('/app/browser')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<BrowserProvider>
			<div>
				<Outlet />
			</div>
		</BrowserProvider>
	);
}

export function BrowserLayout(props: any) {
	return (
		<div className="relative h-full flex flex-1 flex-col items-center gap-2">
			<div className="h-full w-full max-w-screen-xl flex flex-1 flex-col items-center gap-y-2">
				<div className="grid grid-cols-[220px_auto_220px] w-full gap-x-6">
					<div />
					<BrowserToolbar />
					<div />
				</div>

				<div className="grid grid-cols-[220px_auto_220px] w-full max-w-screen-xl gap-x-6 pb-8">
					<BrowserCategories />

					<div className="h-full flex flex-col gap-y-4">
						<div className="h-full flex-1">
							{props.children}
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
	return (
		<div className="flex flex-col gap-y-4">
			<div className="flex flex-col gap-y-4">
				<div className="flex flex-col gap-y-1">
					<h6 className="my-1">Active Cluster</h6>
					<Dropdown
						onSelectionChange={(id) => {
							const cluster = clusters?.find(cluster => cluster.id.toString() === id);
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
					<h6 className="my-1">Provider</h6>
					<Dropdown>
						{PROVIDERS.map(provider => (
							<Dropdown.Item key={provider}>
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

function BrowserCategories() {
	const categories = browserCategories.byPackageType('mod', 'Modrinth');

	return (
		<div className="top-0 grid grid-cols-[1fr_auto] h-fit min-w-50 gap-y-6">
			<div />
			<div className="flex flex-col gap-y-6">
				<Show when>
					<div className="flex flex-col gap-y-2">
						<h6 className="my-1">Categories</h6>
						{categories.map(category => (
							<p
								className="text-md capitalize opacity-100 hover:opacity-90 text-fg-primary hover:text-fg-primary-hover"
								key={category.id}
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
	return (
		<div className="w-full flex flex-row justify-between bg-page">
			<div className="flex flex-row gap-2" />

			<div className="flex flex-row gap-2">
				<TextField
					iconLeft={<SearchMdIcon />}
					placeholder="Search for content"
				/>
			</div>
		</div>
	);
}
