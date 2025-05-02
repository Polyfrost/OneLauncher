import { ArrowRightIcon, Server01Icon } from '@untitled-theme/icons-solid';
import Button from '~ui/components/base/Button';
import { type Accessor, type Context, createContext, createSignal, type JSX, type ParentProps, type Setter, Show, useContext } from 'solid-js';
import { createStore } from 'solid-js/store';
import HeaderImage from '../../../../assets/images/header.png';
import { createModal } from '../Modal';
import ClusterGameSetup from './ClusterGameSetup';
import ClusterImportSelection from './ClusterImportSelection';
import ClusterProviderSelection, { type ClusterCreationProvider } from './ClusterProviderSelection';

export enum CreationStage {
	PROVIDER_SELECTION,
	GAME_SETUP,
	IMPORT_SELECTION,
}

type FinishFn = () => Promise<boolean> | boolean;

interface ClusterModalController {
	step: Accessor<CreationStage | undefined>;
	setStep: (step: CreationStage | number | undefined) => void;

	provider: Accessor<ClusterCreationProvider | undefined>;
	setProvider: Setter<ClusterCreationProvider | undefined>;

	finishFunction: Accessor<FinishFn>;
	setFinishFunction: Setter<FinishFn>;

	start: () => void;
	previous: () => void;
	cancel: () => void;
	finish: () => void;
}

const ClusterModalContext = createContext<ClusterModalController>() as Context<ClusterModalController>;

export function ClusterModalControllerProvider(props: ParentProps) {
	const [provider, setProvider] = createSignal<ClusterCreationProvider | undefined>();
	const [finishFunction, setFinishFunction] = createSignal<FinishFn>(() => true);
	const [steps, setSteps] = createSignal<CreationStage[]>([]);
	const [stepComponents] = createStore<{ [key in CreationStage]: () => JSX.Element }>({
		[CreationStage.PROVIDER_SELECTION]: ClusterProviderSelection,
		[CreationStage.GAME_SETUP]: ClusterGameSetup,
		[CreationStage.IMPORT_SELECTION]: ClusterImportSelection,
	});

	const controller: ClusterModalController = {
		step: () => {
			return steps()[steps().length - 1];
		},
		setStep: (stage) => {
			setSteps((prev) => {
				if (stage === undefined)
					return [];

				if (stage < 0) {
					const next = [...prev];
					next.pop();
					return next;
				}

				return [...prev, stage];
			});
		},

		provider,
		setProvider,

		finishFunction,
		setFinishFunction,

		start() {
			controller.setStep(CreationStage.PROVIDER_SELECTION);
			modal.show();
		},

		previous() {
			controller.setStep(-1);
		},

		cancel() {
			controller.setStep(undefined);
			modal.hide();
		},

		finish() {
			const fn = finishFunction()();

			if (fn instanceof Promise)
				fn.then((res) => {
					if (res === true)
						modal.hide();
				});
			else
				if (fn === true)
					modal.hide();
		},
	};

	const modal = createModal(() => (
		<Show when={controller.step() !== undefined}>
			<>{stepComponents[controller.step()!]()}</>
		</Show>
	));

	return (
		<ClusterModalContext.Provider value={controller}>
			{props.children}
		</ClusterModalContext.Provider>
	);
}

export function useClusterCreator() {
	const ctx = useContext(ClusterModalContext);

	if (!ctx)
		throw new Error('useClusterCreator must be used within a ClusterModalControllerProvider');

	return ctx;
}

type ButtonType = 'next' | 'create';

export interface ClusterStep {
	stage: CreationStage;
	component: (props: ClusterStepProps) => JSX.Element;
}

export interface ClusterStepProps {
	setCanGoForward: Setter<boolean>;
	setNextStage: Setter<CreationStage | undefined>;
	controller: ClusterModalController;
};

interface CreateClusterStepType {
	message: string;
	buttonType: ButtonType;
	Component: (props: ClusterStepProps) => JSX.Element;
}

export function createClusterStep(props: CreateClusterStepType): () => JSX.Element {
	return () => {
		const controller = useClusterCreator();
		const [canGoForward, setCanGoForward] = createSignal(false);
		const [nextStage, setNextStage] = createSignal<CreationStage | undefined>();

		const previousButtonText = () => {
			if (controller.step() === CreationStage.PROVIDER_SELECTION)
				return 'Cancel';
			else
				return 'Previous';
		};

		function previousBtnClick() {
			if (controller.step() === CreationStage.PROVIDER_SELECTION)
				controller.cancel();
			else
				controller.previous();
		}

		function btnClick() {
			if (props.buttonType === 'create')
				controller.finish();
			else if (props.buttonType === 'next')
				controller.setStep(nextStage());
		}

		return (
			<div class="min-w-sm flex flex-col rounded-lg bg-page text-center">
				<div class="theme-OneLauncher-Dark relative h-25 flex">
					<div class="absolute left-0 top-0 h-full w-full">
						<img alt="Header Image" class="h-full w-full rounded-t-lg" src={HeaderImage} />
					</div>
					<div
						class="absolute left-0 top-0 h-full flex flex-row items-center justify-start gap-x-4 bg-[radial-gradient(at_center,#00000077,transparent)] px-10"
					>
						<Server01Icon class="h-8 w-8 text-fg-primary" />
						<div class="flex flex-col items-start justify-center">
							<h1 class="h-10 text-fg-primary -mt-2">New Cluster</h1>
							<span class="text-fg-primary">{props.message}</span>
						</div>
					</div>
				</div>
				<div class="flex flex-col border border-white/5 rounded-b-lg">
					<div class="p-3">
						<props.Component
							controller={controller}
							setCanGoForward={setCanGoForward}
							setNextStage={setNextStage}
						/>
					</div>

					<div class="flex flex-row justify-end gap-x-2 p-3 pt-0">
						<Button
							buttonStyle="ghost"
							children={previousButtonText()}
							class="capitalize"
							onClick={previousBtnClick}
						/>

						<Button
							buttonStyle="primary"
							children={props.buttonType}
							class="capitalize"
							disabled={canGoForward() !== true}
							iconRight={<ArrowRightIcon />}
							onClick={btnClick}
						/>
					</div>
				</div>
			</div>
		);
	};
}
