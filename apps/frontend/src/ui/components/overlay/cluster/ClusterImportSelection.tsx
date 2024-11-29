import type { ImportType } from '@onelauncher/client/bindings';
import { bridge } from '~imports';
import SelectList from '~ui/components/base/SelectList';
import useCommand from '~ui/hooks/useCommand';
import { createEffect, createSignal, For, onMount, untrack } from 'solid-js';
import { type ClusterStepProps, createClusterStep } from './ClusterCreationModal';

export default createClusterStep({
	message: 'Import Launcher',
	buttonType: 'create',
	Component: ClusterImportSelection,
});

function ClusterImportSelection(props: ClusterStepProps) {
	const [instances] = useCommand(() => {
		const importType = props.controller.provider();
		if (importType === undefined || importType === 'New')
			return [];

		return bridge.commands.getLauncherInstances(importType, null);
	});

	const [selected, setSelected] = createSignal<number>();

	createEffect(() => {
		const index = selected();

		props.setCanGoForward(() => {
			if (index === undefined)
				return false;

			if (index >= 0)
				return true;

			return false;
		});
	});

	onMount(() => {
		// eslint-disable-next-line solid/reactivity -- It's fine
		props.controller.setFinishFunction(() => async () => {
			const index = untrack(selected);
			if (index === undefined)
				return false;

			const list = untrack(instances);
			if (list === undefined)
				return false;

			const instance = list[1][index];
			if (instance === undefined)
				return false;

			const basePath = list[0];

			const importType = untrack(props.controller.provider) as ImportType;

			await bridge.commands.importInstances(importType, basePath, [instance]);

			return true;
		});
	});

	return (
		<SelectList
			class="h-52 max-h-52"
			onChange={selected => setSelected(selected[0])}
		>
			<For each={instances()?.[1]}>
				{(instance, index) => (
					<SelectList.Row index={index()}>
						{instance}
					</SelectList.Row>
				)}
			</For>
		</SelectList>
	);
}
