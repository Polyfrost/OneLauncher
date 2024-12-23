import type { ImportType } from '@onelauncher/client/bindings';
import type { LauncherImportInformation } from '~ui/components/content/LauncherImportComponent';
import { bridge } from '~imports';
import LauncherImportComponent from '~ui/components/content/LauncherImportComponent';
import { createSignal, onMount, untrack } from 'solid-js';
import { type ClusterStepProps, createClusterStep } from './ClusterCreationModal';

export default createClusterStep({
	message: 'Import Launcher',
	buttonType: 'create',
	Component: ClusterImportSelection,
});

function ClusterImportSelection(props: ClusterStepProps) {
	const [importInfo, setImportInfo] = createSignal<LauncherImportInformation>({
		// eslint-disable-next-line solid/reactivity -- It works as intended
		importType: untrack(props.controller.provider) as ImportType,
		instances: [],
		path: null,
	});

	onMount(() => {
		props.controller.setFinishFunction(() => async () => {
			const details = untrack(importInfo);

			bridge.commands.importInstances(details.importType, details.path!, details.instances);

			return true;
		});
	});

	return (
		<LauncherImportComponent
			importInformation={importInfo}
			multiple={false}
			setCanImport={props.setCanGoForward}
			setImportInformation={setImportInfo}
		/>
	);
}
