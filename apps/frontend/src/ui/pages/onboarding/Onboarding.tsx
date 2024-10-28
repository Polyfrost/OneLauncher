import type { ImportType } from '@onelauncher/client/bindings';
import { Route, useBeforeLeave, useLocation, useNavigate } from '@solidjs/router';
import { ChevronLeftIcon, ChevronRightIcon } from '@untitled-theme/icons-solid';
import AnimatedRoutes from '~ui/components/AnimatedRoutes';
import Button from '~ui/components/base/Button';
import useSettings from '~ui/hooks/useSettings';
import { setAsyncTimeout } from '~utils';
import { type Accessor, type Context, createContext, createEffect, createSignal, type JSX, on, type ParentProps, useContext } from 'solid-js';
import OnboardingComplete from './OnboardingComplete';
import OnboardingImport from './OnboardingImport';
import OnboardingLanguage, { type Language, LanguagesList } from './OnboardingLanguage';
import OnboardingSummary from './OnboardingSummary';
import OnboardingWelcome from './OnboardingWelcome';

const basePath = '/onboarding';
const OnboardingSteps = [
	['/', OnboardingWelcome],

	['/language', OnboardingLanguage],
	['/import', OnboardingImport],

	['/summary', OnboardingSummary], // Second to last will always be the summary (which is basically a confirmation for all the tasks that are about to be done)
	['/complete', OnboardingComplete], // Last will always be a "onelauncher is ready to use" page
] as const;

// eslint-disable-next-line no-restricted-syntax -- -
export const enum OnboardingTaskStage {
	NotStarted = 0,
	Running = 1,
	Completed = 2,
}

interface ImportInstancesType {
	basePath: string;
	instances: string[];
}

type InstancesImportMapType = Map<ImportType, ImportInstancesType>;

interface OnboardingContextType {
	setLanguage: (language: Language) => void;
	language: () => Language;

	setImportInstances: (type: ImportType, basePath: string, instances: string[]) => void;
	importInstances: (type: ImportType) => ImportInstancesType | undefined;

	getTasks: () => string[];

	tasksStage: Accessor<OnboardingTaskStage>;
	tasksMessage: Accessor<string>;
}

const OnboardingContext = createContext() as Context<OnboardingContextType>;

function Onboarding(props: ParentProps) {
	const navigate = useNavigate();
	const location = useLocation();
	const { settings, save: saveSettings } = useSettings();

	const [backButtonEnabled, setBackButtonEnabled] = createSignal(false);
	const [forwardButtonEnabled, setForwardButtonEnabled] = createSignal(true);

	const [language, setLanguage] = createSignal<Language>('en');
	const [importInstances, setImportInstances] = createSignal<InstancesImportMapType>(new Map());
	const [tasksStage, setTasksStage] = createSignal<OnboardingTaskStage>(OnboardingTaskStage.NotStarted);
	const [tasksMessage, setTasksMessage] = createSignal<string>('');

	const tasksCompleted = () => tasksStage() === OnboardingTaskStage.Completed;

	const percentage = () => (step() / (OnboardingSteps.length - 1)) * 100;
	const step = () => OnboardingSteps.findIndex(([path]) => (basePath + path).startsWith(location.pathname));
	const shallRunTasks = () => !tasksCompleted() && step() === OnboardingSteps.length - 2;

	createEffect(on(() => location.pathname, () => {
		setBackButtonEnabled(step() > 0 && !tasksCompleted());
	}));

	useBeforeLeave((e) => {
		if (tasksCompleted() && step() === OnboardingSteps.length - 1 && !settings().onboarding_completed)
			e.preventDefault();
	});

	const next = () => {
		const isLast = step() === OnboardingSteps.length - 1;

		if (isLast)
			saveSettings({
				...settings(),
				onboarding_completed: true,
			}).then(() => navigate('/'));
		else
			navigate(basePath + OnboardingSteps[step() + 1]![0]);
	};

	const previous = () => {
		if (step() > 0)
			navigate(basePath + OnboardingSteps[step() - 1]![0]);
	};

	const getButtonText = () => {
		if (shallRunTasks())
			return 'Setup';
		else if (step() === OnboardingSteps.length - 1)
			return 'Finish';
		else
			return 'Next';
	};

	const runTasks = async () => {
		setBackButtonEnabled(false);
		setForwardButtonEnabled(false);

		setTasksStage(OnboardingTaskStage.Running);

		const tasks = [

			async () => {
				setTasksMessage('Setting language');
				await setAsyncTimeout(500);
			},

			async () => {
				setTasksMessage('Importing data');
				await setAsyncTimeout(1000);
			},

		].map(task => task());

		const errorCount = (await Promise.allSettled(tasks)).filter(({ status }) => status === 'rejected').length;

		if (errorCount > 0)
			console.error('Error count:', errorCount); // TODO: Show a toast or something idk

		setForwardButtonEnabled(true);
		setTasksStage(OnboardingTaskStage.Completed);
		next(); // Go to Completion page
	};

	const getTasks = () => {
		const tasks = [];

		tasks.push(`Set language to ${LanguagesList[language()][0]}`);

		importInstances().forEach((type) => {
			tasks.push(`Import profiles from ${type}`);
		});

		return tasks;
	};

	const ctx: OnboardingContextType = {
		setLanguage,
		language,

		setImportInstances(type, basePath, instances) {
			setImportInstances((importTypes) => {
				const newMap = new Map(importTypes);
				newMap.set(type, {
					basePath,
					instances,
				});
				return newMap;
			});
		},
		importInstances: type => importInstances().get(type),

		getTasks,

		tasksStage,
		tasksMessage,
	};

	return (
		<OnboardingContext.Provider value={ctx}>
			<div class="h-full max-h-full w-full flex flex-col items-center justify-center">
				<div class="h-0.5 w-full">
					<div
						class="h-full rounded-lg bg-brand transition-all"
						style={{
							width: `${percentage()}%`,
						}}
					/>
				</div>

				<div class="h-full max-w-280 w-full flex flex-col gap-y-4 p-8">
					<div class="h-full w-full">
						<AnimatedRoutes>
							{props.children}
						</AnimatedRoutes>
					</div>

					<div class="z-1 w-full flex flex-1 flex-row items-end justify-end">
						<div class="w-1/3 flex flex-row items-stretch gap-x-8 [&>*]:w-full">
							<Button
								buttonStyle="secondary"
								children="Previous"
								disabled={!backButtonEnabled()}
								iconLeft={<ChevronLeftIcon />}
								onClick={previous}
							/>

							<Button
								children={getButtonText()}
								disabled={!forwardButtonEnabled()}
								iconRight={<ChevronRightIcon />}
								onClick={shallRunTasks() ? runTasks : next}
							/>
						</div>
					</div>
				</div>
			</div>
		</OnboardingContext.Provider>
	);
}

Onboarding.useContext = () => {
	const ctx = useContext(OnboardingContext);
	if (!ctx)
		throw new Error('Onboarding context not found');

	return ctx;
};

Onboarding.Steps = OnboardingSteps;
Onboarding.Routes = OnboardingSteps.map(([path, component]) => (
	<Route component={component} path={path} />
));

export default Onboarding;

export interface OnboardingStepProps extends ParentProps {
	title: string;
	paragraph: string;
	illustration: JSX.Element;
}

export function OnboardingStep(props: OnboardingStepProps) {
	return (
		<div class="grid grid-cols-2 h-full w-full gap-x-16">
			<div class="flex flex-col items-center justify-center">
				{props.illustration}
			</div>

			<div class="flex flex-col justify-center gap-y-4">
				<div class="w-full flex flex-col gap-y-2">
					<h1 class="text-2xl">{props.title}</h1>
					<p class="text-lg text-fg-secondary line-height-normal">{props.paragraph}</p>
				</div>

				<div class="max-h-96 w-full flex flex-1 flex-col gap-y-2">
					{props.children}
				</div>
			</div>
		</div>
	);
}
