import type { LogViewerRef } from '@/components';
import { LoaderContainer, LogViewer, SheetPage, useSheetPageContext } from '@/components';
import { bindings } from '@/main';
import { getMessageFromError, useAsyncEffect, useCommand } from '@onelauncher/common';
import { createFileRoute } from '@tanstack/react-router';
import { useRef } from 'react';

export const Route = createFileRoute('/app/cluster/process')({
	component: RouteComponent,
});

function RouteComponent() {
	const { scrollContainerRef } = useSheetPageContext();

	return (
		<SheetPage.Content>
			<div className="flex flex-col gap-4">
				<div className="flex justify-between items-center">
					<h2 className="flex-1 text-xxl font-semibold">Game Log</h2>
				</div>

				<LogContent scrollRef={scrollContainerRef} />
			</div>
		</SheetPage.Content>
	);
}

function LogContent({
	scrollRef,
}: {
	scrollRef: React.RefObject<HTMLElement | null>;
}) {
	const { cluster } = Route.useRouteContext();
	const logViewerRef = useRef<LogViewerRef>(null);

	const { data: tail, error, isLoading, refetch } = useCommand(
		['getProcessLogTail', cluster.id, 0],
		() => bindings.core.getProcessLogTail(cluster.id, 0),
		{
			staleTime: 0,
			gcTime: 0,
			refetchOnWindowFocus: false,
			refetchOnReconnect: false,
		},
	);

	useAsyncEffect(async () => {
		const unlisten = await bindings.events.process.on((e) => {
			if (e.cluster_id !== cluster.id)
				return;

			if (e.kind.type === 'Started') {
				refetch();
				return;
			}

			if (e.kind.type !== 'Output')
				return;

			const outputLines = e.kind.output.split('\n');
			for (const line of outputLines)
				logViewerRef.current?.push(line);
		});

		return () => {
			unlisten();
		};
	}, []);

	return (
		<LoaderContainer loading={isLoading}>
			<LogViewer
				autoScroll
				content={getMessageFromError(error) || tail?.content || 'Empty log file'}
				ref={logViewerRef}
				scrollRef={scrollRef}
			/>
		</LoaderContainer>
	);
}
