import { type Accessor, type Context, type JSX, type ParentProps, type Setter, Show, createContext, createSignal, untrack, useContext } from 'solid-js';
import { ArrowRightIcon, Server01Icon } from '@untitled-theme/icons-solid';
import { createStore } from 'solid-js/store';
import HeaderImage from '../../../../assets/images/header.png';
import { createModal } from '../Modal';
import ClusterGameSetup from './ClusterGameSetup';
import ClusterProviderSelection from './ClusterProviderSelection';
import Button from '~ui/components/base/Button';
import type { CreateCluster } from '~bindings';

export enum CreationStage {
	PROVIDER_SELECTION = 0,
	GAME_SETUP = 1,
	IMPORT_SELECTION = 2,
}

type PartialCluster = Partial<CreateCluster>;
type PartialClusterUpdateFunc = <K extends keyof PartialCluster>(key: K, value: PartialCluster[K]) => any;

interface ClusterModalController {
	step: Accessor<CreationStage | undefined>;
	setStep: (step: CreationStage | number | undefined) => void;

	partialCluster: Accessor<PartialCluster>;
	setPartialCluster: Setter<PartialCluster>;
	updatePartialCluster: PartialClusterUpdateFunc;

	start: () => Promise<void>;
	previous: () => void;
	cancel: () => void;
	finish: () => void;
}

const ClusterModalContext = createContext() as Context<ClusterModalController>;

export function ClusterModalControllerProvider(props: ParentProps) {
	const [partialCluster, setPartialCluster] = createSignal<PartialCluster>({});
	const requiredProps: (keyof PartialCluster)[] = ['name', 'mc_version', 'mod_loader'];

	const [steps, setSteps] = createSignal<CreationStage[]>([]);
	const [stepComponents] = createStore<{ [key in CreationStage]: () => JSX.Element }>({
		[CreationStage.PROVIDER_SELECTION]: ClusterProviderSelection,
		[CreationStage.GAME_SETUP]: ClusterGameSetup,
		[CreationStage.IMPORT_SELECTION]: () => <></>,
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

		partialCluster,
		setPartialCluster,
		updatePartialCluster: (key, value) => {
			setPartialCluster((prev) => {
				return {
					...prev,
					[key]: value,
				};
			});
		},

		async start() {
			setPartialCluster({});
			controller.setStep(CreationStage.PROVIDER_SELECTION);
			// eslint-disable-next-line ts/no-use-before-define -- It should still work
			modal.show();
		},

		previous() {
			controller.setStep(-1);
		},

		cancel() {
			controller.setStep(undefined);
			// eslint-disable-next-line ts/no-use-before-define -- It should still work
			modal.hide();
		},

		async finish() {
			const untracked = untrack(partialCluster);

			for (const prop of requiredProps)
				if (!untracked[prop])
					throw new Error(`Missing required property ${prop}`);

			// @ts-expect-error -- TODO: Do this properly
			await bridge.commands.createCluster({
				icon: null,
				icon_url: null,
				loader_version: null,
				package_data: null,
				skip: null,
				skip_watch: null,
				...untracked,
			});
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
			<div class="min-w-sm flex flex-col rounded-lg bg-primary text-center">
				<div class="relative h-25 flex">
					<div class="absolute left-0 top-0 h-full w-full">
						<img class="h-full w-full rounded-t-lg" src={HeaderImage} alt="Header Image" />
					</div>
					<div
						class="absolute left-0 top-0 h-full flex flex-row items-center justify-start gap-x-4 bg-[radial-gradient(at_center,#00000077,transparent)] px-10"
					>
						<Server01Icon class="h-8 w-8" />
						<div class="flex flex-col items-start justify-center">
							<h1 class="h-10 -mt-2">New Cluster</h1>
							<span>{props.message}</span>
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
							class="capitalize"
							children={previousButtonText()}
							buttonStyle="ghost"
							onClick={previousBtnClick}
						/>

						<Button
							children={props.buttonType}
							class="capitalize"
							buttonStyle="primary"
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
