import type { Key } from 'react-aria-components';
import { LoaderContainer, SheetPage, useSheetPageContext } from '@/components';
import { LogViewer } from '@/components/LogViewer';
import { bindings } from '@/main';
import { getMessageFromError, useCommand, useCommandSuspense } from '@onelauncher/common';
import { Dropdown } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { useState } from 'react';

export const Route = createFileRoute('/app/cluster/logs')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();
	const { scrollContainerRef } = useSheetPageContext();

	const { data: fileNames } = useCommandSuspense(['getLogs', cluster.id], () => bindings.core.getLogs(cluster.id));

	const [activeFileName, setActiveFileName] = useState<Key | undefined>(fileNames[0] || undefined);

	return (
		<SheetPage.Content>
			<div className="flex flex-col gap-4">
				<div className="flex justify-between items-center">
					<h2 className="flex-1 text-xxl font-semibold">Logs</h2>

					<div className="flex flex-row">
						<Dropdown onSelectionChange={e => setActiveFileName(e)} selectedKey={activeFileName}>
							{fileNames.map(fileName => (
								<Dropdown.Item id={fileName} key={fileName}>
									{fileName}
								</Dropdown.Item>
							))}
						</Dropdown>
					</div>
				</div>

				{activeFileName && (<LogContent fileName={activeFileName.toString()} scrollRef={scrollContainerRef} />)}
			</div>
		</SheetPage.Content>
	);
}

function LogContent({
	fileName,
	scrollRef,
}: {
	fileName: string;
	scrollRef: React.RefObject<HTMLElement | null>;
}) {
	const { cluster } = Route.useRouteContext();

	const { data: content, error, isLoading } = useCommand(
		['getLogByName', cluster.id, fileName],
		() => bindings.core.getLogByName(cluster.id, fileName),
		{
			gcTime: 1000 * 60 * 1, // 1 minute
		},
	);

	return (
		<LoaderContainer loading={isLoading}>
			<LogViewer content={getMessageFromError(error) || content || 'Empty log file'} scrollRef={scrollRef} />
		</LoaderContainer>
	);
}
