import type { ImportType } from '@onelauncher/client/bindings';
import { RefreshCw01Icon } from '@untitled-theme/icons-solid';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import FilePicker from '~ui/components/base/FilePicker';
import SelectList from '~ui/components/base/SelectList';
import { tryResult } from '~ui/hooks/useCommand';
import useNotifications from '~ui/hooks/useNotifications';
import { type Accessor, createEffect, createResource, createSignal, For, type Setter, untrack } from 'solid-js';
import Spinner from '../Spinner';

export interface LauncherImportInformation {
	importType: ImportType;
	path: string | null;
	instances: string[];
};

interface LauncherImportComponentProps {
	multiple: boolean;
	setCanImport?: Setter<boolean>;
	importInformation: Accessor<LauncherImportInformation>;
	setImportInformation: Setter<LauncherImportInformation>;
};

export default function LauncherImportComponent(props: LauncherImportComponentProps) {
	const notifications = useNotifications();

	// eslint-disable-next-line solid/reactivity -- Its fine
	const [selected, setSelected] = createSignal<string[]>(props.importInformation().instances || []);
	// eslint-disable-next-line solid/reactivity -- Its fine
	const [customPath, setCustomPath] = createSignal<string | null>(props.importInformation().path || null);
	const [pathChanged, setPathChanged] = createSignal<boolean>(false);

	const [instances, { refetch: refetchInstances }] = createResource(async () => {
		const info = props.importInformation();
		const importType = info.importType;
		const path = untrack(customPath) || null; // empty path

		try {
			const result = await tryResult(() => bridge.commands.getLauncherInstances(importType, path));
			setCustomPath(result[0]);

			return result[1];
		}
		catch (err) {
			// If the path is not null this means that the launcher's default location was not found
			if (path !== null)
				notifications.create({
					title: 'Instance Scan Error',
					message: `${err}`,
				});

			return [];
		}
	});

	createEffect(() => {
		const selectedInstances = selected();

		props.setCanImport?.(() => selectedInstances.length > 0);

		props.setImportInformation({
			importType: untrack(props.importInformation).importType,
			path: untrack(customPath)!,
			instances: selectedInstances,
		});
	});

	function onPicked(paths: string[]) {
		setCustomPath(paths[0] || '');
		setPathChanged(true);
	}

	async function refresh() {
		refetchInstances();
		setPathChanged(false);
	}

	return (
		<div class="max-w-sm w-full flex flex-col gap-2">
			<Spinner.Suspense>
				<h4>Path</h4>
				<div class="max-w-full w-full flex flex-row items-center justify-center gap-2">
					<FilePicker
						defaultPath={customPath() || ''}
						directory
						onPicked={onPicked}
						ref={(ref) => {
							if (customPath() === null)
								ref.open();
						}}
					/>
					<Button
						buttonStyle="iconSecondary"
						children={<RefreshCw01Icon />}
						disabled={!pathChanged()}
						onClick={refresh}
					/>
				</div>

				<h4 class="mt-2">Found Instances</h4>
				<SelectList
					class="h-52 max-h-52"
					multiple={props.multiple}
					onChange={(indexes) => {
						const mapped = instances()?.map((instance, index) => indexes.includes(index) ? instance : undefined).filter((instance): instance is string => !!instance) || [];
						setSelected(mapped);
					}}
				>
					<For each={instances()}>
						{(instance, index) => {
						// console.log(props.importInformation(), props.importInformation().instances.includes(instance));
							return (
								<SelectList.Row index={index()} selected={selected().includes(instance)}>
									{instance}
								</SelectList.Row>
							);
						}}
					</For>
				</SelectList>
			</Spinner.Suspense>
		</div>
	);
}
