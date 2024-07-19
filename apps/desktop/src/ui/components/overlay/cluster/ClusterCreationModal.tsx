import { type Accessor, type Context, Match, type ParentProps, type Setter, Show, Switch, createContext, createSignal, useContext } from 'solid-js';
import { Server01Icon } from '@untitled-theme/icons-solid';
import Modal, { type ModalProps } from '../Modal';
import HeaderImage from '../../../../assets/images/header.png';

// Why the fuck do I need to use a context for all this???
// TODO: Rewrite for use with the new Modal stacking system
type ClusterModalContextFunc = [
    step: Accessor<number>,
    setStep: Setter<number>,
    setVisible: Setter<boolean>,
];

const ClusterModalContext = createContext<ClusterModalContextFunc>() as Context<ClusterModalContextFunc>;

export function ClusterModalController(props: ParentProps) {
	const [step, setStep] = createSignal(1);
	const [visible, setVisible] = createSignal(false);

	const stepper: ClusterModalContextFunc = [
		step,
		setStep,
		setVisible,
	];

	return (
		<ClusterModalContext.Provider value={stepper}>
			{props.children}
			<ClusterCreationModal visible={visible} setVisible={setVisible} step={step()} />
		</ClusterModalContext.Provider>
	);
}

export function useClusterModalController() {
	return useContext(ClusterModalContext);
}

type ClusterCreationModalProps = ModalProps & {
	step: number;
};

function ClusterCreationModal(props: ClusterCreationModalProps) {
	// const StepComponent = () => (
	// 	<Switch>
	// 		<Match when={step() === 1}>
	// 			<ClusterStepOne />
	// 		</Match>
	// 	</Switch>
	// );

	return (
		<Modal {...props}>
			<div class="relative h-22">
				<div class="absolute top-0 left-0 w-full">
					<img src={HeaderImage} alt="Header Image" />
				</div>
				<div class="absolute top-0 left-0 w-full flex flex-row justify-center items-center gap-x-2">
					<Server01Icon />
					<div class="flex flex-col">
						<h1>New Cluster</h1>
						<span>agagwag</span>
					</div>
				</div>
			</div>
		</Modal>
	);
}
