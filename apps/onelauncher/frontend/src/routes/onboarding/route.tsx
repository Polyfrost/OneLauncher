/* eslint-disable no-console -- we dont have a seperate logger for now */
import type { ReactNode } from 'react';
import { pluralize } from '@/utils';
import { Button } from '@onelauncher/common/components';
import { createFileRoute, Outlet, useNavigate, useRouterState } from '@tanstack/react-router';
import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';

export const Route = createFileRoute('/onboarding')({
	component: RouteComponent,
});

const steps = [
	'/onboarding',
	'/onboarding/language',
	'/onboarding/login',
	'/onboarding/import',
	'/onboarding/summary',
	'/onboarding/complete',
] as const;

interface Language {
	lang: string;
	code: string;
	percentage: number;
}

export const languageList: Array<Language> = [
	{
		lang: 'English',
		code: 'en',
		percentage: 100,
	},
];

enum OnboardingTaskStage {
	NotStarted = 0,
	Running = 1,
	Completed = 2,
}

interface ImportInstancesType {
	basePath: string;
	instances: Array<string>;
}

interface OnboardingContextType {
	setLanguage: (language: Language) => void;
	language: Language | undefined;

	setImportInstances: (type: string, basePath: string, instances: Array<string>) => void;
	getImportInstance: (type: string) => ImportInstancesType | undefined;

	setIsForwardButtonEnabled: (enabled: boolean) => void;

	getTasks: () => Array<string>;

	tasksStage: OnboardingTaskStage;
	tasksMessage: string;
}

const OnboardingContext = createContext<OnboardingContextType | undefined>(undefined);

export function useOnboardingContext() {
	const context = useContext(OnboardingContext);
	if (context === undefined)
		throw new Error('useOnboardingContext must be used within an OnboardingProvider (RouteComponent)');

	return context;
}

function RouteComponent() {
	const navigate = useNavigate({ from: Route.fullPath });
	const pathname = useRouterState({ select: s => s.location.pathname });

	const currentStepIndex = useMemo(() => {
		return steps.findIndex(stepPath => stepPath === pathname);
	}, [pathname]);

	const [isBackButtonEnabled, setIsBackButtonEnabled] = useState(false);
	const [isForwardButtonEnabled, setIsForwardButtonEnabled] = useState(true);

	const [language, setLanguage] = useState<Language>();
	const [importInstancesMap, setImportInstancesMap] = useState<Map<string, ImportInstancesType>>(new Map());
	const [tasksStage, setTasksStage] = useState<OnboardingTaskStage>(OnboardingTaskStage.NotStarted);
	const [tasksMessage, setTasksMessage] = useState<string>('');

	const tasksCompleted = tasksStage === OnboardingTaskStage.Completed;

	const percentage = (currentStepIndex / (steps.length - 1)) * 100;

	const shallRunTasks = !tasksCompleted && currentStepIndex === steps.length - 2;

	useEffect(() => {
		const isSummaryStep = currentStepIndex === steps.length - 2;
		const isCompleteStep = currentStepIndex === steps.length - 1;

		setIsBackButtonEnabled(currentStepIndex > 0 && tasksStage !== OnboardingTaskStage.Running);

		if (tasksStage === OnboardingTaskStage.Running)
			setIsForwardButtonEnabled(false);
		else if (tasksCompleted)
			setIsForwardButtonEnabled(isSummaryStep || isCompleteStep);
		else
			setIsForwardButtonEnabled(true);
	}, [currentStepIndex, tasksStage, tasksCompleted]);

	const next = useCallback(async () => {
		const isLast = currentStepIndex === steps.length - 1;

		if (isLast) {
			navigate({ to: '/app' });
		}
		else {
			const nextPath = steps[currentStepIndex + 1];

			navigate({ to: nextPath });
		}
	}, [currentStepIndex, navigate]);

	const previous = useCallback(() => {
		if (currentStepIndex > 0) {
			const prevPath = steps[currentStepIndex - 1];

			navigate({ to: prevPath });
		}
	}, [currentStepIndex, navigate]);

	const getButtonText = useMemo(() => {
		if (shallRunTasks)
			return 'Setup';
		if (currentStepIndex === steps.length - 1)
			return 'Finish';
		return 'Next';
	}, [shallRunTasks, currentStepIndex]);

	const runTasks = useCallback(async () => {
		setIsBackButtonEnabled(false);
		setIsForwardButtonEnabled(false);
		setTasksStage(OnboardingTaskStage.Running);
		setTasksMessage('Starting setup...');

		const tasksToRun = [
			async () => {
				setTasksMessage(`Setting language to ${language?.lang}...`);

				console.log('Language task completed for:', language?.lang);
			},
			async () => {
				if (importInstancesMap.size === 0) {
					console.log('No instances selected for import.');
					return;
				}

				for (const [launcher, importData] of importInstancesMap.entries()) {
					if (importData.instances.length === 0)
						continue;

					setTasksMessage(`Importing ${importData.instances.length} ${pluralize(importData.instances.length, 'instance')} from ${launcher}...`);
					try {
						console.log(`Simulating import for ${launcher}:`, importData);
					}
					catch (e) {
						console.error(`Failed to import from ${launcher}:`, e);
						setTasksMessage(`Failed to import ${pluralize(importData.instances.length, 'instance')} from ${launcher}...`);
						throw e;
					}
				}
			},
		];

		const results = await Promise.allSettled(tasksToRun.map(task => task()));
		const errorCount = results.filter(({ status }) => status === 'rejected').length;

		if (errorCount > 0) {
			console.error('Onboarding tasks completed with error count:', errorCount);
			setTasksMessage(`Setup completed with ${errorCount} ${pluralize(errorCount, 'error')}. Check console for details.`);
		}
		else {
			setTasksMessage('Setup completed successfully!');
		}

		setTasksStage(OnboardingTaskStage.Completed);
		// setIsBackButtonEnabled(true);
		setIsForwardButtonEnabled(true);
		next();
	}, [importInstancesMap, next, language?.lang, setIsBackButtonEnabled, setIsForwardButtonEnabled, setTasksStage, setTasksMessage]);

	const getTasks = useCallback((): Array<string> => {
		const tasksStrings: Array<string> = [];
		const selectedLangInfo = languageList.find(l => l.lang === language?.lang);
		const langDisplayName = selectedLangInfo
			? `${selectedLangInfo.lang} (Coverage: ${selectedLangInfo.percentage}%)`
			: language?.lang;
		tasksStrings.push(`Set language to ${langDisplayName}`);

		importInstancesMap.forEach((importData, launcher) => {
			if (importData.instances.length === 0)
				return;

			let builder = `Import ${importData.instances.length} ${pluralize(importData.instances.length, 'instance')} from ${launcher} (Source: ${importData.basePath}):\n`;
			importData.instances.forEach((instance, index) => {
				builder += `  ${index + 1}. ${instance}\n`;
			});
			tasksStrings.push(builder.trim());
		});

		return tasksStrings;
	}, [language, importInstancesMap]);

	const handleSetImportInstances = useCallback((type: string, newBasePath: string, instances: Array<string>) => {
		setImportInstancesMap((prevMap) => {
			const newMap = new Map(prevMap);
			newMap.set(type, { basePath: newBasePath, instances });
			return newMap;
		});
	}, []);

	const getImportInstance = useCallback((type: string) => {
		return importInstancesMap.get(type);
	}, [importInstancesMap]);

	const ctxValue: OnboardingContextType = useMemo(() => ({
		setLanguage,
		language,
		setImportInstances: handleSetImportInstances,
		getImportInstance,
		setIsForwardButtonEnabled,
		getTasks,
		tasksStage,
		tasksMessage,
	}), [
		language,
		handleSetImportInstances,
		getImportInstance,
		setIsForwardButtonEnabled,
		getTasks,
		tasksStage,
		tasksMessage,
	]);

	return (
		<OnboardingContext value={ctxValue}>
			<div className="w-full flex flex-col items-center h-screen bg-background-primary text-text-primary">
				<div className="h-0.5 w-full">
					<div
						className="h-full rounded-lg bg-brand transition-all duration-300 ease-in-out"
						style={{
							width: `${percentage}%`,
						}}
					/>
				</div>

				<div className="flex-1 max-w-5xl w-full flex flex-col gap-y-4 p-8 overflow-y-auto">
					<Outlet />
				</div>

				<div className="w-full max-w-5xl p-8">
					<div className="w-1/3 flex flex-row items-stretch gap-x-8 [&>*]:w-full ml-auto">
						<Button color="secondary" isDisabled={!isBackButtonEnabled} onClick={previous}>Back</Button>
						<Button color="primary" isDisabled={!isForwardButtonEnabled} onClick={shallRunTasks ? runTasks : next}>
							{getButtonText}
						</Button>
					</div>
				</div>
			</div>
		</OnboardingContext>
	);
}

export interface OnboardingStepProps {
	title: string;
	paragraph: string;
	illustration?: ReactNode;
	children: ReactNode;
}

export function OnboardingStep({
	title,
	paragraph,
	illustration,
	children,
}: OnboardingStepProps) {
	return (
		<div className={`grid ${illustration ? 'grid-cols-1 md:grid-cols-2' : 'grid-cols-1'} h-full w-full gap-x-16`}>
			{illustration && (
				<div className="hidden md:flex flex-col items-center justify-center p-4">
					{illustration}
				</div>
			)}
			<div className="flex flex-col justify-center gap-y-4">
				<div className="w-full flex flex-col gap-y-2">
					<h1 className="text-3xl font-semibold text-text-primary">{title}</h1>
					<p className="text-lg text-text-secondary leading-relaxed">{paragraph}</p>
				</div>
				<div className="max-h-[30rem] w-full flex-1 flex flex-col gap-y-2 overflow-y-auto pr-2">
					{children}
				</div>
			</div>
		</div>
	);
}
