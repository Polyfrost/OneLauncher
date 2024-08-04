import { type Accessor, type Context, Match, type ParentProps, type Setter, Switch, createContext, createEffect, createSignal, onCleanup, untrack, useContext } from 'solid-js';
import { ArrowRightIcon, Server01Icon } from '@untitled-theme/icons-solid';
import HeaderImage from '../../../../assets/images/header.png';
import { createModal } from '../Modal';
import ClusterStepOne from './ClusterStepOne';
import { ClusterStepTwo } from './ClusterStepTwo';
import Button from '~ui/components/base/Button';
import type { CreateCluster } from '~bindings';
import { bridge } from '~imports';
import type { FixedArray, UnionToArray } from '~types.ts';

type PartialCluster = Partial<CreateCluster>;
type PartialClusterUpdateFunc = <K extends keyof PartialCluster>(key: K, value: PartialCluster[K]) => any;

// Why the fuck do I need to use a context for all this???
// TODO: Rewrite for use with the new Modal stacking system
interface ClusterModalContextFunc {
	step: Accessor<number>;
	setStep: Setter<number>;
	partialCluster: Accessor<PartialCluster>;
	setPartialCluster: Setter<PartialCluster>;
	updatePartialCluster: PartialClusterUpdateFunc;
	start: () => Promise<void>; // Returns true if its finished (created) successfully
	finish: () => void;
}

const ClusterModalContext = createContext<ClusterModalContextFunc>() as Context<ClusterModalContextFunc>;

export enum ClusterModalStages {
	STAGE_1_PROVIDER = 0,
	STAGE_2_GAME_SETUP = 1,
}

type ClusterModalStagesLength = UnionToArray<keyof typeof ClusterModalStages>['length'];

export function ClusterModalController(props: ParentProps) {
	const [step, setStep] = createSignal<number>(ClusterModalStages.STAGE_1_PROVIDER);
	const [partialCluster, setPartialCluster] = createSignal<PartialCluster>({});
	const [inProgress, setInProgress] = createSignal(false);
	const [error, setError] = createSignal<string | undefined>(undefined);

	const updatePartialCluster: PartialClusterUpdateFunc = (key, value) => {
		setPartialCluster((prev) => {
			return {
				...prev,
				[key]: value,
			};
		});
	};

	const modal = createModal(() => (
		// eslint-disable-next-line ts/no-use-before-define -- circular dependency should be refactored later
		<ClusterCreationModal controller={context} />
	));

	const context: ClusterModalContextFunc = {
		step,
		setStep,
		partialCluster,
		setPartialCluster,
		updatePartialCluster,

		start() {
			setPartialCluster({});
			setStep(ClusterModalStages.STAGE_1_PROVIDER);
			setError(undefined);
			setInProgress(true);
			modal.show();

			return new Promise<void>((resolve, reject) => {
				createEffect(() => {
					const result = error();
					if (inProgress() === true)
						return;

					if (result === undefined || result.length === 0)
						resolve();
					else
						reject(result);
				});
			});
		},

		async finish() {
			const untracked = untrack(partialCluster);

			if (untracked.name === undefined || untracked.mc_version === undefined || untracked.mod_loader === undefined)
				throw new Error('Cluster is missing required fields');

			modal.hide();

			await bridge.commands.createCluster({
				name: untracked.name!,
				mod_loader: untracked.mod_loader!,
				mc_version: untracked.mc_version!,
				// TODO: Implement the rest of the fields
				icon: null,
				icon_url: null,
				loader_version: null,
				package_data: null,
				skip: null,
				skip_watch: null,
			});

			setInProgress(false);
		},
	};

	onCleanup(() => {
		setStep(ClusterModalStages.STAGE_1_PROVIDER);
	});

	return (
		<ClusterModalContext.Provider value={context}>
			{props.children}
		</ClusterModalContext.Provider>
	);
}

export function useClusterModalController() {
	const ctx = useContext(ClusterModalContext);

	if (!ctx)
		throw new Error('useClusterModalController must be used within a ClusterModalController');

	return ctx;
}

export interface ClusterStepProps {
	isVisible: Accessor<boolean>;
	setCanGoForward: Setter<boolean>;
	controller: Accessor<ClusterModalContextFunc>;
};

function ClusterCreationModal(props: { controller: ClusterModalContextFunc }) {
	const buttonIsNext = () => props.controller.step() !== ((Object.keys(ClusterModalStages).length / 2) - 1);

	const [canGoForward, setCanGoForward] = createSignal(false);
	// Indexed by the current step
	const messages: FixedArray<string, ClusterModalStagesLength> = [
		'Provider selection',
		'Game Setup',
	];

	const stepComponents = [
		ClusterStepOne,
		ClusterStepTwo,
	].map((Component, index) => (
		Component({
			setCanGoForward,
			isVisible: () => props.controller.step() === index,
			controller: () => props.controller,
		})
	));

	const Step = (props: { step: number }) => <>{stepComponents[props.step]}</>;

	function cancel() {
		// controller.setVisible(false);
	}

	function prev() {
		props.controller.setStep(prev => prev - 1);
	}

	function next() {
		props.controller.setStep(prev => prev + 1);
	}

	function finish() {
		props.controller.finish();
	}

	return (
		<div class="bg-primary rounded-lg text-center flex flex-col min-w-sm">
			<div class="flex relative h-25">
				<div class="absolute top-0 left-0 w-full h-full">
					<img class="h-full w-full rounded-t-lg" src={HeaderImage} alt="Header Image" />
				</div>
				<div
					class="absolute top-0 left-0 px-10 h-full flex flex-row justify-start items-center gap-x-4 bg-[radial-gradient(at_center,#00000077,transparent)]"
				>
					<Server01Icon class="w-8 h-8" />
					<div class="flex flex-col items-start justify-center">
						{/** weird positioning?? */}
						<h1 class="h-10 -mt-2">New Cluster</h1>
						<span>{messages[props.controller.step()]}</span>
					</div>
				</div>
			</div>
			<div class="flex flex-col border border-white/5 rounded-b-lg">
				<div class="p-3">
					<Step step={props.controller.step()} />
				</div>

				<div class="flex flex-row gap-x-2 justify-end pt-0 p-3">
					<Switch>
						<Match when={props.controller.step() === 0}>
							<Button
								children="Cancel"
								buttonStyle="ghost"
								onClick={cancel}
							/>
						</Match>
						<Match when={props.controller.step() >= 1}>
							<Button
								children="Previous"
								buttonStyle="ghost"
								onClick={prev}
							/>
						</Match>
					</Switch>

					<Switch>
						<Match when={buttonIsNext() === true}>
							<Button
								children="Next"
								buttonStyle="primary"
								disabled={!canGoForward()}
								iconRight={<ArrowRightIcon />}
								onClick={next}
							/>
						</Match>
						<Match when={buttonIsNext() === false}>
							<Button
								children="Create"
								buttonStyle="primary"
								disabled={!canGoForward()}
								iconRight={<ArrowRightIcon />}
								onClick={finish}
							/>
						</Match>
					</Switch>
				</div>
			</div>
		</div>
	);
}
