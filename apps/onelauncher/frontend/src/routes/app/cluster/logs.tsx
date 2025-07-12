import type { ProcessPayload } from '@/bindings.gen';
import ScrollableContainer from '@/components/ScrollableContainer';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Button, Show } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { dataDir, join } from '@tauri-apps/api/path';
import { openPath } from '@tauri-apps/plugin-opener';
import { LinkExternal01Icon, Upload01Icon } from '@untitled-theme/icons-react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useEffect } from 'react';
import { proxy, useSnapshot } from 'valtio';
import Sidebar from '../settings/route';

export const Route = createFileRoute('/app/cluster/logs')({
	component: RouteComponent,
});

const store = proxy({
	logs: [] as Array<ProcessPayload>,
});

function RouteComponent() {
	const { id: _id } = Route.useSearch();
	const cluster = useCommand('getClusterById', () => bindings.core.getClusterById(Number(_id.toString()) as unknown as bigint));
	const state = useSnapshot(store);

	const addLog = (newLog: ProcessPayload) => {
		store.logs.push(newLog);
	};

	useEffect(() => {
		let cleanup: (() => void) | undefined;

		bindings.events.process.on(addLog).then((e) => {
			cleanup = e;
		}).catch(console.error);

		return () => {
			cleanup?.();
		};
	}, []);

	const openLogsFolder = async () => {
		openPath(await join(await dataDir(), 'OneLauncher', 'clusters', cluster.data?.folder_name as string, 'logs'));
	};

	return (
		<Sidebar.Page>
			<ScrollableContainer>
				<div className="h-full">
					<div className="flex flex-1 flex-col">
						<div className="flex flex-1 flex-col gap-y-2">
							<div className="h-10 flex flex-row items-center justify-between gap-x-1">
								<h1>Logs</h1>
								<div className="flex flex-row gap-x-2">
									<Button
										color="secondary"
									// isDisabled={missingLogs()}
									// onClick={uploadAndOpenLog}
									>
										<Upload01Icon />
										{' '}
										Upload
									</Button>

									{/* <Dropdown
                class="min-w-50"
                disabled={missingLogs()}
                onChange={changeLog}
              >
                <For each={logs() || ['None']}>
                  {(log) => {
                    const pretty = log.split('/').pop();
                    return (
                      <Dropdown.Row>
                        <div>
                          {pretty}
                        </div>
                      </Dropdown.Row>
                    );
                  }}
                </For>
              </Dropdown> */}

									<Button
										color="primary"
										onClick={openLogsFolder}
									>
										<LinkExternal01Icon />
										{' '}
										Open Folder
									</Button>
								</div>
							</div>

							<Show
								fallback={<span>No logs were found.</span>}
								when={state.logs.length > 0}
							>
								<FormattedLog log={state.logs} />
								{/* {JSON.stringify(state.logs)} */}
							</Show>
						</div>
					</div>
				</div>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

interface FormattedLogProps {
	log: ReadonlyArray<ProcessPayload>;
}

function FormattedLog(props: FormattedLogProps) {
	const { log } = props;

	return (
		<div className="relative h-full flex flex-1 flex-col overflow-hidden rounded-lg bg-component-bg border border-component-border shadow-sm">

			<div className="h-px bg-gradient-to-r from-transparent via-component-border to-transparent" />

			<div className="h-full flex flex-1 overflow-auto font-medium font-mono text-sm">
				<OverlayScrollbarsComponent
					className="relative h-full flex-1"
				>
					<code className="log-container whitespace-pre flex flex-col text-sm p-2 gap-0.5">
						{log.map((logLine, index) => {
							if (index === log.length - 1)
								return null;

							// trickery fuckery
							const lineKey = `${logLine.type}-${index}-${(logLine.type === 'Output' ? logLine.output : '').slice(0, 50)}`;
							return <Line key={lineKey} line={logLine} />;
						})}
					</code>
				</OverlayScrollbarsComponent>
			</div>
		</div>
	);
}

const REGEX_PATTERN = /\[(\d+:\d+:\d+)\] \[(.[^(\n\r/\u2028\u2029]*)\/(\w+)\]:? (?:\[(CHAT)\])?/;

export function Line(props: { line: ProcessPayload }) {
	const { line } = props;

	const format = (logText: string) => {
		let processedLine = logText.replace(/§./g, '');
		const prefix = processedLine.match(REGEX_PATTERN);

		const isEmpty = processedLine.trim() === '';

		if (isEmpty)
			processedLine = '\n';

		if (prefix === null)
			return (
				<span
					className={`
						block py-0.5 rounded-sm hover:bg-component-bg-hover transition-colors duration-150 select-text
						text-fg-secondary leading-relaxed
						${isEmpty ? 'h-5 hover:bg-transparent' : ''}
					`}
				>
					{processedLine}
				</span>
			);

		const isChatMsg = prefix[4] === 'CHAT';
		const logLevel = prefix[3].toUpperCase();

		const getLevelColor = (level: string) => {
			switch (level) {
				case 'INFO':
					return 'text-code-info';
				case 'DEBUG':
					return 'text-code-debug';
				case 'TRACE':
					return 'text-code-trace';
				case 'WARN':
					return 'text-code-warn';
				case 'ERROR':
					return 'text-code-error';
				default:
					return 'text-fg-primary';
			}
		};

		return (
			<span
				className={`
					block py-0.5 rounded-sm hover:bg-component-bg-hover transition-colors duration-150 select-text
					${getLevelColor(logLevel)} leading-relaxed
					${isEmpty ? 'h-5 hover:bg-transparent' : ''}
				`}
				data-level={logLevel}
				{...(isChatMsg ? { 'data-chat': 'true' } : {})}
				{...(isEmpty ? { 'data-empty': 'true' } : {})}
			>
				<span className="text-fg-secondary opacity-60 font-medium text-xs mr-2 inline-block min-w-[4.5rem] text-right">
					{prefix[1]}
				</span>
				<span className="font-semibold opacity-90 text-xs mr-2">
					{`[${prefix[2]}/${prefix[3]}]`}
				</span>
				{isChatMsg && (
					<span className="bg-brand text-white px-1.5 py-0.5 rounded text-xs font-medium mr-2">
						CHAT
					</span>
				)}
				<span className={`${isChatMsg ? 'text-white font-medium' : ''} break-words`}>
					{processedLine.slice(prefix[0].length)}
				</span>
			</span>
		);
	};

	return (
		<>{format(line.type === 'Output' ? line.output : '')}</>
	);
}
