import type { Key } from 'react-aria-components';
import { SheetPage, useSheetPageContext } from '@/components';
import { LogViewer } from '@/components/LogViewer';
import { bindings } from '@/main';
import { useCommand, useCommandSuspense } from '@onelauncher/common';
import { Dropdown } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { useEffect, useState } from 'react';

export const Route = createFileRoute('/app/cluster/logs')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();
	const { scrollContainerRef } = useSheetPageContext();

	const { data: fileNames } = useCommandSuspense(['getLogs', cluster.id.toString()], () => bindings.core.getLogs(cluster.id));

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

	// TODO: Fix cache
	const { data: content } = useCommand(
		['getLogByName', cluster.id.toString(), fileName],
		() => bindings.core.getLogByName(cluster.id, fileName),
		{
			gcTime: 1000 * 60 * 1, // 1 minute
		},
	);

	return (
		content && <LogViewer content={content} scrollRef={scrollRef} />
	);
}
