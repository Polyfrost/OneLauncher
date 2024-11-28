import type { ImportType } from '@onelauncher/client/bindings';
import Illustration from '~assets/illustrations/onboarding/import_from_others.svg?component-solid';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import Link from '~ui/components/base/Link';
import SelectList from '~ui/components/base/SelectList';
import LauncherIcon from '~ui/components/content/LauncherIcon';
import Modal, { createModal, type ModalProps } from '~ui/components/overlay/Modal';
import useCommand from '~ui/hooks/useCommand';
import { LAUNCHER_IMPORT_TYPES } from '~utils';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { createEffect, createSignal, For } from 'solid-js';
import Onboarding, { OnboardingStep } from './Onboarding';

function OnboardingImport() {
	const ctx = Onboarding.useContext();
	const [launcher, setLauncher] = createSignal<ImportType>();

	const modal = createModal(props => (
		<InstancesPickerModal
			{...props}
			launcher={launcher()!}
			onSelected={(basePath, instance) => {
				ctx.setImportInstances(launcher()!, basePath, instance);
			}}
			selected={ctx.importInstances(launcher()!)?.instances}
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
	selected: string[] | undefined;
	onSelected: (basePath: string, instances: string[]) => void;
};

function InstancesPickerModal(props: InstancesPickerModalProps) {
	const [instances] = useCommand(() => bridge.commands.getLauncherInstances(props.launcher, null));
	const [selected, setSelected] = createSignal<number[]>([]);

	const getSelectedInstances = () => selected().map(i => instances()?.[1][i]).filter(v => v !== undefined);

	createEffect(() => {
		const selected = props.selected;
		const instancesList = instances()?.[1];
		if (selected !== undefined && instancesList !== undefined) {
			const indexes = selected.map(instance => instancesList.indexOf(instance)).filter(i => i !== undefined);
			setSelected(indexes);
		}
	});

	return (
		<Modal.Simple
			{...props}
			buttons={[
				'Cancel',
				<Button
					buttonStyle="primary"
					children={`Import (${selected().length || 'None'})`}
					disabled={(instances()?.length || 0) <= 0}
					onClick={() => {
						props.onSelected(instances()![0], getSelectedInstances());
						props.hide();
					}}
				/>,
			]}
			title="Import Instances"
		>
			<div class="flex flex-col gap-2">
				<SelectList
					class="h-52 max-h-52"
					multiple
					onChange={setSelected}
					selected={selected()}
				>
					<For each={instances()?.[1]}>
						{(instance, index) => (
							<SelectList.Row index={index()}>
								{instance}
							</SelectList.Row>
						)}
					</For>
				</SelectList>
			</div>
		</Modal.Simple>
	);
}
