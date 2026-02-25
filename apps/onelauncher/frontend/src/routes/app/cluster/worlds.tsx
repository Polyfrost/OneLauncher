import WorldIcon from '@/components/content/WorldIcon';
import ScrollableContainer from '@/components/ScrollableContainer';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Button, Show } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { Trash01Icon } from '@untitled-theme/icons-react';
import Sidebar from '../settings/route';

export const Route = createFileRoute('/app/cluster/worlds')({
	component: RouteComponent,
});

function RouteComponent() {
	const { id } = Route.useSearch();
	const cluster = useCommand('getClusterById', () => bindings.core.getClusterById(Number(id.toString()) as unknown as bigint));
	const list = useCommand('getWorlds', () => bindings.core.getWorlds(Number(id.toString()) as unknown as bigint));

	return (
		<Sidebar.Page>
			<h1>Worlds</h1>
			<ScrollableContainer>
				<div className="h-full">
					<Show
						fallback={<div className="text-border/400">No worlds found.</div>}
						when={list.data.length > 0}
					>
						<div className="flex flex-col gap-2">
							{list.data.map(data => (
								<WorldEntry cluster_path={cluster.data?.folder_name || ''} key={data} name={data} />
							))}
						</div>
					</Show>
				</div>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

function WorldEntry(props: { name: string; cluster_path: string }) {
	const { name, cluster_path } = props;

	return (
		<div
			className="flex flex-row items-center justify-between gap-3 rounded-xl bg-component-bg p-3 active:bg-component-bg-pressed hover:bg-component-bg-hover"
		>
			<div className="flex flex-row items-center gap-x-3">
				<WorldIcon className="aspect-ratio-square h-16 w-16" cluster_name={cluster_path} world_name={name} />
				<div className="flex flex-col gap-y-2">
					<h3>{name}</h3>
					<p>Todo</p>
				</div>
			</div>

			<div className="flex flex-row items-center justify-end gap-x-3">
				<Button
					children={<Trash01Icon />}
					color="danger"
				/>
			</div>
		</div>
	);
}
