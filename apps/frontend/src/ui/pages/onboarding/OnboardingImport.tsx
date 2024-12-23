import type { ImportType } from '@onelauncher/client/bindings';
import type { LauncherImportInformation } from '~ui/components/content/LauncherImportComponent';
import Illustration from '~assets/illustrations/onboarding/import_from_others.svg?component-solid';
import Button from '~ui/components/base/Button';
import Link from '~ui/components/base/Link';
import LauncherIcon from '~ui/components/content/LauncherIcon';
import LauncherImportComponent from '~ui/components/content/LauncherImportComponent';
import Modal, { createModal, type ModalProps } from '~ui/components/overlay/Modal';
import { LAUNCHER_IMPORT_TYPES } from '~utils';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { createMemo, createSignal, For, untrack } from 'solid-js';
import Onboarding, { OnboardingStep } from './Onboarding';

function OnboardingImport() {
	const ctx = Onboarding.useContext();
	const [launcher, setLauncher] = createSignal<ImportType>();

	const foundInfo = createMemo(() => ctx.importInstances(launcher()!));

	const modal = createModal(props => (
		<InstancesPickerModal
			{...props}
			instances={foundInfo()?.instances}
			launcher={launcher()!}
			onSelected={(basePath, instance) => {
				ctx.setImportInstances(launcher()!, basePath, instance);
			}}
			path={foundInfo()?.basePath}
		/>
	));

	function displayImport(type: ImportType) {
		setLauncher(type);
		modal.show();
	}

	return (
		<OnboardingStep
			illustration={<Illustration />}
			paragraph="Import your profiles from other launchers."
			title="Import"
		>
			<div class="h-full w-full flex flex-col gap-y-3">
				<OverlayScrollbarsComponent>
					<div class="grid grid-cols-3">
						<For each={LAUNCHER_IMPORT_TYPES}>
							{type => (
								<button
									class={`flex flex-col items-center justify-center gap-y-4 rounded-md p-4 active:bg-border/10 hover:bg-border/05 ${(ctx.importInstances(type)?.instances.length || 0) > 0 ? 'bg-success hover:bg-success/70' : ''}`}
									onClick={() => displayImport(type)}
								>
									<LauncherIcon class="h-16 max-w-22 min-w-16" launcher={type} />
									<span class="text-lg font-medium">{type}</span>
								</button>
							)}
						</For>
					</div>
					<small class="pt-2 text-fg-secondary">
						Want to contribute a launcher import? Click
						{' '}
						<Link href="https://github.com/Polyfrost/OneLauncher" skipPrompt={true}>here</Link>
						.
					</small>
				</OverlayScrollbarsComponent>
			</div>
		</OnboardingStep>
	);
}

export default OnboardingImport;

type InstancesPickerModalProps = ModalProps & {
	launcher: ImportType;
	path: string | undefined;
	instances: string[] | undefined;
	onSelected: (basePath: string, instances: string[]) => void;
};

function InstancesPickerModal(props: InstancesPickerModalProps) {
	const [importInfo, setImportInfo] = createSignal<LauncherImportInformation>({
		// eslint-disable-next-line solid/reactivity -- its ok
		importType: props.launcher,
		// eslint-disable-next-line solid/reactivity -- its ok
		instances: props.instances || [],
		// eslint-disable-next-line solid/reactivity -- its ok
		path: props.path || '',
	});

	const instanceLength = createMemo(() => importInfo().instances.length);

	return (
		<Modal.Simple
			{...props}
			buttons={[
				'Cancel',
				<Button
					buttonStyle="primary"
					children={`Import (${instanceLength() || 'None'})`}
					disabled={instanceLength() <= 0}
					onClick={() => {
						const info = untrack(importInfo);
						props.onSelected(info.path!, info.instances);
						props.hide();
					}}
				/>,
			]}
			title="Import Instances"
		>
			<LauncherImportComponent
				importInformation={importInfo}
				multiple={true}
				setImportInformation={setImportInfo}
			/>
		</Modal.Simple>
	);
}
