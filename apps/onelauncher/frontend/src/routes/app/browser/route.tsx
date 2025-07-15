import { BrowserProvider, useBrowserContext } from '@/hooks/useBrowser';
import { useClusters } from '@/hooks/useCluster';
import { PROVIDERS } from '@/utils';
import { Button, Dropdown, Show, TextField } from '@onelauncher/common/components';
import { createFileRoute, Outlet } from '@tanstack/react-router';
import { SearchMdIcon } from '@untitled-theme/icons-react';

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
					<Dropdown onSelectionChange={provider => context.setProvider(provider as typeof context.provider)} selectedKey={context.provider}>
						{/* <For each={PROVIDERS}>
							{provider => (
								<Dropdown.Row>
									<ProviderIcon class="h-4 w-4" provider={provider} />
									{provider}
								</Dropdown.Row>
							)}
						</For> */}
						{PROVIDERS.map(provider => (
							<Dropdown.Item id={provider} key={provider}>
								{provider}
							</Dropdown.Item>
						))}
					</Dropdown>
				</div>
			</div>
		</div>
	);
}

function BrowserCategories() {
	return (
		<div className="top-0 grid grid-cols-[1fr_auto] h-fit min-w-50 gap-y-6">
			<div />
			<div className="flex flex-col gap-y-6">
				<Show when>
					<div className="flex flex-col gap-y-2">
						<h6 className="my-1">Categories</h6>
						{/* <For each={categories()}>
              {category => (
                <p
                  class={`text-md capitalize text-fg-primary hover:text-fg-primary-hover line-height-snug ${isEnabled(category.id) ? 'text-opacity-100! hover:text-opacity-90!' : 'text-opacity-60! hover:text-opacity-70!'}`}
                  onClick={() => toggleCategory(category.id)}
                >
                  {category.display}
                </p>
              )}
            </For> */}
						<p
							className="text-md capitalize opacity-100 hover:opacity-90 text-fg-primary hover:text-fg-primary-hover"
						>
							Jennie
						</p>
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
