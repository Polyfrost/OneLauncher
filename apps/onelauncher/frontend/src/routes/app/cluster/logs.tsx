import type { ProcessPayload } from '@/bindings.gen';
import ScrollableContainer from '@/components/ScrollableContainer';
import { bindings } from '@/main';
import { Button, Show } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { LinkExternal01Icon, Upload01Icon } from '@untitled-theme/icons-react';
import { useCallback, useEffect, useReducer } from 'react';
import {
	ListBox,
	ListBoxItem,
	ListLayout,
	Virtualizer,
} from 'react-aria-components';
import Sidebar from '../settings/route';

export const Route = createFileRoute('/app/cluster/logs')({
	component: RouteComponent,
});

enum LoggerActionType {
	ADD,
}

interface LoggerAction {
	type: LoggerActionType;
	payload: ProcessPayload;
}

function reducer(state: Array<ProcessPayload>, action: LoggerAction) {
	const { type, payload } = action;
	switch (type) {
		// eslint-disable-next-line ts/no-unnecessary-condition -- oh please shut the fuck up i dont want to listen your yapping anymore
		case LoggerActionType.ADD:
			return [...state, payload];
		default:
			return state;
	}
}

function RouteComponent() {
	const { id: _id } = Route.useSearch();
	const [logs, dispatch] = useReducer(reducer, []);

	const addLog = useCallback((newLog: ProcessPayload) => {
		dispatch({ type: LoggerActionType.ADD, payload: newLog });
	}, []);

	useEffect(() => {
		bindings.events.process.on(addLog);
	}, [addLog]);

	return (
		<Sidebar.Page>
			<h1>Overview</h1>
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
										// onClick={openFolder}
									>
										<LinkExternal01Icon />
										{' '}
										Open Folder
									</Button>
								</div>
							</div>

							<Show
								fallback={<span>No logs were found.</span>}
								when={logs.length > 0}
							>
								<FormattedLog log={logs} />
							</Show>
						</div>
					</div>
				</div>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

interface FormattedLogProps {
	log: Array<ProcessPayload>;
}

function FormattedLog(props: FormattedLogProps) {
	const { log } = props;

	return (
		<>
			<Virtualizer
				layout={ListLayout}
				layoutOptions={{
					rowHeight: 32,
					padding: 4,
					gap: 4,
				}}
			>
				<ListBox
					aria-label="Virtualized ListBox"
					items={log}
					selectionMode="none"
				>
					{(item) => {
						if (item.type === 'Output')
							return (
								<ListBoxItem key={item.output}>
									{item.output}
								</ListBoxItem>
							);
					}}
				</ListBox>
			</Virtualizer>
		</>
	);
}
