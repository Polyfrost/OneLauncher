import { type Accessor, type Context, Match, type ParentProps, type Setter, Switch, createContext, createEffect, createSignal, useContext } from 'solid-js';
import { ArrowRightIcon, Server01Icon } from '@untitled-theme/icons-solid';
import HeaderImage from '../../../../assets/images/header.png';
import FullscreenOverlay, { type FullscreenOverlayProps } from '../FullscreenOverlay';
import ClusterStepOne from './ClusterStepOne';
import { ClusterStepTwo } from './ClusterStepTwo';
import Button from '~ui/components/base/Button';

// Why the fuck do I need to use a context for all this???
// TODO: Rewrite for use with the new Modal stacking system
type ClusterModalContextFunc = [
    step: Accessor<number>,
    setStep: Setter<number>,
    setVisible: Setter<boolean>,
];

const ClusterModalContext = createContext<ClusterModalContextFunc>() as Context<ClusterModalContextFunc>;

export enum ClusterModalStages {
	STAGE_1_PROVIDER = 0,
	STAGE_2_GAME_SETUP = 1,
}

type ClusterModalStagesLength = UnionToArray<keyof typeof ClusterModalStages>['length'];

export function ClusterModalController(props: ParentProps) {
	const [step, setStep] = createSignal<number>(ClusterModalStages.STAGE_1_PROVIDER);
	const [visible, setVisible] = createSignal(false);
	const [canGoForward, setCanGoForward] = createSignal(false);

	const stepper: ClusterModalContextFunc = [
		step,
		setStep,
		setVisible,
	];

	createEffect(() => {
		if (visible() === false)
			setStep(ClusterModalStages.STAGE_1_PROVIDER);
	});

	const stepComponents = [
		ClusterStepOne,
		ClusterStepTwo,
	];

	return (
		<ClusterModalContext.Provider value={stepper}>
			{props.children}
			<ClusterCreationModal
				visible={visible}
				setVisible={setVisible}
				step={step}
				setStep={setStep}
				canGoForward={canGoForward}
				buttonIsNext={step() !== ((Object.keys(ClusterModalStages).length / 2) - 1)}
			>
				{(stepComponents[step()]!({
					canGoForward, setCanGoForward,
				}))}
			</ClusterCreationModal>
		</ClusterModalContext.Provider>
	);
}

export function useClusterModalController() {
	return useContext(ClusterModalContext);
}

export interface ClusterStepProps {
	canGoForward: Accessor<boolean>;
	setCanGoForward: Setter<boolean>;
};

type ClusterCreationModalProps = FullscreenOverlayProps & {
	step: Accessor<number>;
	setStep: Setter<number>;
	buttonIsNext: boolean;
	canGoForward: Accessor<boolean>;
};

function ClusterCreationModal(props: ClusterCreationModalProps) {
	// Indexed by the current step
	const messages: FixedArray<string, ClusterModalStagesLength> = [
		'Provider selection',
		'Game Setup',
	];

	return (
		<FullscreenOverlay
			visible={props.visible}
			setVisible={props.setVisible}
			mount={props.mount}
			zIndex={props.zIndex || 1000}
		>
			<div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2">
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
								<span>{messages[props.step()]}</span>
							</div>
						</div>
					</div>
					<div class="flex flex-col border border-white/5 rounded-b-lg">
						<div class="p-3">
							{props.children}
						</div>

						<div class="flex flex-row gap-x-2 justify-end pt-0 p-3">
							<Switch>
								<Match when={props.step() === 0}>
									<Button
										children="Cancel"
										buttonStyle="ghost"
										onClick={() => props.setVisible(false)}
									/>
								</Match>
								<Match when={props.step() >= 1}>
									<Button
										children="Previous"
										buttonStyle="ghost"
										onClick={() => props.setStep(prev => prev - 1)}
									/>
								</Match>
							</Switch>

							<Switch>
								<Match when={props.buttonIsNext === true}>
									<Button
										children="Next"
										buttonStyle="primary"
										disabled={!props.canGoForward()}
										iconRight={<ArrowRightIcon />}
										onClick={() => props.setStep(prev => prev + 1)}
									/>
								</Match>
								<Match when={props.buttonIsNext === false}>
									<Button
										children="Create"
										buttonStyle="primary"
										disabled={!props.canGoForward()}
										iconRight={<ArrowRightIcon />}
									/>
								</Match>
							</Switch>
						</div>
					</div>
				</div>
			</div>
		</FullscreenOverlay>
	);
}
