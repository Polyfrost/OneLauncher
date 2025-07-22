import type { GameLoader, Version } from '@/bindings.gen';
import type { JSX, RefAttributes } from 'react';
import DefaultBanner from '@/assets/images/default_banner.png';
import LauncherIcon from '@/components/content/LauncherIcon';
import Modal from '@/components/overlay/Modal';
import ScrollableContainer from '@/components/ScrollableContainer';
import { bindings } from '@/main';
import { LAUNCHER_IMPORT_TYPES, LOADERS, upperFirst } from '@/utils';
import { useCommand, useCommandMut } from '@onelauncher/common';
import { Button, Dropdown, SelectList, TextField } from '@onelauncher/common/components';
import { useQueryClient } from '@tanstack/react-query';
import { ArrowRightIcon, PlusIcon, SearchMdIcon, Server01Icon, TextInputIcon, User03Icon } from '@untitled-theme/icons-react';
import { useCallback, useMemo, useState } from 'react';
import { Checkbox } from 'react-aria-components';
import LoaderIcon from '../LoaderIcon';

const STEPS = [
	{
		id: 'loader' as const,
		title: 'Select Provider',
	},
	{
		id: 'info' as const,
		title: 'Game Setup',
	},
] as const;

interface ClusterFormData {
	loader: GameLoader;
	clusterName: string;
	clusterVersion: string;
	clusterProvider: string;
}

interface FormValidation {
	isProviderValid: boolean;
	isNameValid: boolean;
	isVersionValid: boolean;
}

export function NewClusterCreate() {
	const queryClient = useQueryClient();
	const [currentStepIndex, setCurrentStepIndex] = useState(0);
	const [isModalOpen, setIsModalOpen] = useState(false);
	const [formData, setFormData] = useState<ClusterFormData>({
		loader: 'vanilla',
		clusterName: '',
		clusterVersion: '',
		clusterProvider: '',
	});

	const validation = useMemo((): FormValidation => ({
		isProviderValid: Boolean(formData.clusterProvider),
		isNameValid: Boolean(formData.clusterName.trim()),
		isVersionValid: Boolean(formData.clusterVersion.trim()),
	}), [formData.clusterProvider, formData.clusterName, formData.clusterVersion]);

	const create = useCommandMut(() => bindings.core.createCluster({
		icon: null,
		mc_loader: formData.loader,
		mc_loader_version: null,
		mc_version: formData.clusterVersion,
		name: formData.clusterName,
	}), {
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ['getClusters'] });
			// Modal'ı kapat ve formu sıfırla
			setIsModalOpen(false);
			setCurrentStepIndex(0);
			setFormData({
				loader: 'vanilla',
				clusterName: '',
				clusterVersion: '',
				clusterProvider: '',
			});
		},
	});

	const handleCreate = useCallback(() => {
		if (!validation.isNameValid || !validation.isVersionValid) {
			console.warn('Form validation failed');
			return;
		}

		create.mutate();
	}, [create, validation.isNameValid, validation.isVersionValid]);

	const handleNext = useCallback(() => {
		if (currentStepIndex === STEPS.length - 1) {
			handleCreate();
			return;
		}

		if (currentStepIndex < STEPS.length - 1)
			setCurrentStepIndex(prev => prev + 1);
	}, [currentStepIndex, handleCreate]);

	const handleBack = useCallback(() => {
		if (currentStepIndex > 0)
			setCurrentStepIndex(prev => prev - 1);
	}, [currentStepIndex]);

	const formHandlers = useMemo(() => ({
		onProviderChange: (provider: string) =>
			setFormData(prev => ({ ...prev, clusterProvider: provider })),
		onLoaderChange: (loader: GameLoader) =>
			setFormData(prev => ({ ...prev, loader })),
		onNameChange: (name: string) =>
			setFormData(prev => ({ ...prev, clusterName: name })),
		onVersionChange: (version: string) =>
			setFormData(prev => ({ ...prev, clusterVersion: version })),
	}), []);

	const currentStepConfig = STEPS[currentStepIndex];

	const stepContent = useMemo((): JSX.Element => {
		switch (currentStepConfig.id) {
			case 'loader':
				return (
					<ClusterLoader
						onSelectProvider={formHandlers.onProviderChange}
						selectedProvider={formData.clusterProvider}
					/>
				);
			case 'info':
				return (
					<ClusterInformation
						clusterLoader={formData.loader}
						clusterName={formData.clusterName}
						clusterVersion={formData.clusterVersion}
						onClusterLoaderChange={formHandlers.onLoaderChange}
						onClusterNameChange={formHandlers.onNameChange}
						onClusterVersionChange={formHandlers.onVersionChange}
					/>
				);
			default:
				return <div>Unknown step</div>;
		}
	}, [currentStepConfig.id, formData, formHandlers]);

	const isFirstStep = currentStepIndex === 0;
	const isLastInputStep = currentStepIndex === STEPS.length - 1;
	const nextButtonText = isLastInputStep ? 'Create' : 'Next';

	const isNextDisabled = useMemo(() => {
		switch (currentStepConfig.id) {
			case 'loader':
				return !validation.isProviderValid;
			case 'info':
				return !validation.isNameValid || !validation.isVersionValid;
			default:
				return false;
		}
	}, [currentStepConfig.id, validation]);

	return (
		<div className="flex flex-row justify-between w-full">
			<div>
				<TextField
					aria-label="Cluster Search"
					className="p-1"
					iconLeft={<SearchMdIcon className="size-4" />}
					placeholder="Search for something..."
				/>
			</div>

			<Button onPress={() => setIsModalOpen(true)}>
				<PlusIcon className="stroke-width-[2.2] size-5" />
				New cluster
			</Button>

			<Modal isDismissable isOpen={isModalOpen} onOpenChange={setIsModalOpen}>
				<div className="min-w-sm flex flex-col rounded-lg bg-page text-center">
					<ModalHeader currentStep={currentStepConfig} />

					<div className="flex flex-col rounded-b-lg border border-white/5">
						<div className="p-3">
							{stepContent}
						</div>

						<ModalFooter
							isFirstStep={isFirstStep}
							isNextDisabled={isNextDisabled}
							nextButtonText={nextButtonText}
							onBack={handleBack}
							onNext={handleNext}
						/>
					</div>
				</div>
			</Modal>
		</div>
	);
}

interface ModalHeaderProps {
	currentStep: typeof STEPS[number];
}

function ModalHeader({ currentStep }: ModalHeaderProps) {
	return (
		<div className="theme-OneLauncher-Dark relative h-25 flex">
			<div className="absolute left-0 top-0 h-full w-full">
				<img
					alt="Header Image"
					className="h-full w-full rounded-t-lg"
					src={DefaultBanner}
				/>
			</div>
			<div className="absolute left-0 top-0 h-full flex w-full flex-row items-center justify-start gap-x-4 bg-[radial-gradient(at_center,#00000077,transparent)] px-10">
				<Server01Icon className="h-8 w-8 text-fg-primary" />
				<div className="flex flex-col items-start justify-center">
					<h1 className="h-10 text-fg-primary text-2xl font-semibold">New Cluster</h1>
					<span className="text-fg-primary">{currentStep.title}</span>
				</div>
			</div>
		</div>
	);
}

interface ModalFooterProps {
	isFirstStep: boolean;
	isNextDisabled: boolean;
	nextButtonText: string;
	onBack: () => void;
	onNext: () => void;
}

function ModalFooter({
	isFirstStep,
	isNextDisabled,
	nextButtonText,
	onBack,
	onNext,
}: ModalFooterProps) {
	return (
		<div className="flex flex-row justify-end gap-x-2 p-3 pt-0">
			<Button
				color="ghost"
				isDisabled={isFirstStep}
				onClick={onBack}
			>
				Previous
			</Button>
			<Button
				color="primary"
				isDisabled={isNextDisabled}
				onClick={onNext}
			>
				{nextButtonText}
				{' '}
				<ArrowRightIcon />
			</Button>
		</div>
	);
}

interface ClusterLoaderProps {
	selectedProvider: string;
	onSelectProvider: (provider: string) => void;
}

function ClusterLoader({ selectedProvider, onSelectProvider }: ClusterLoaderProps) {
	const providers = useMemo(() => [
		{ id: 'new', name: 'New', icon: <User03Icon /> },
		...LAUNCHER_IMPORT_TYPES.map(data => ({
			id: data,
			name: data,
			icon: <LauncherIcon launcher={data} />,
		})),
	], []);

	return (
		<div className="grid grid-cols-3 gap-2">
			{providers.map(provider => (
				<ProviderCard
					icon={provider.icon}
					key={provider.id}
					name={provider.name}
					onSelect={() => onSelectProvider(provider.id)}
					selected={selectedProvider === provider.id}
				/>
			))}
		</div>
	);
}

interface ProviderCardProps {
	onSelect: () => void;
	selected: boolean;
	icon: JSX.Element;
	name: string;
}

function ProviderCard({ selected, onSelect, icon, name }: ProviderCardProps) {
	const cardClassName = useMemo(() => {
		const baseClasses = 'flex flex-col justify-center items-center gap-y-3 py-2 px-4 hover:bg-component-bg-hover active:bg-component-bg-pressed rounded-lg cursor-pointer transition-colors';
		return selected ? `${baseClasses} bg-component-bg` : baseClasses;
	}, [selected]);

	return (
		<div
			className={cardClassName}
			onClick={onSelect}
			onKeyDown={(e) => {
				if (e.key === 'Enter' || e.key === ' ') {
					e.preventDefault();
					onSelect();
				}
			}}
			role="button"
			tabIndex={0}
		>
			<div className="h-8 w-8 flex items-center justify-center [&>svg]:(w-8 h-8!)">
				{icon}
			</div>
			<span className="text-sm font-medium">{name}</span>
		</div>
	);
}

interface ClusterInformationProps {
	clusterName: string;
	onClusterNameChange: (name: string) => void;
	clusterVersion: string;
	onClusterVersionChange: (version: string) => void;
	clusterLoader: GameLoader;
	onClusterLoaderChange: (loader: GameLoader) => void;
}

function ClusterInformation({
	clusterName,
	onClusterNameChange,
	clusterVersion,
	onClusterVersionChange,
	clusterLoader,
	onClusterLoaderChange,
}: ClusterInformationProps) {
	const versions = useCommand('getGameVersions', bindings.core.getGameVersions);

	return (
		<div className="flex flex-col gap-y-4">
			<Option header="Name">
				<TextField
					aria-label="Cluster Name"
					className="w-full"
					iconLeft={<TextInputIcon className="size-4" />}
					onChange={e => onClusterNameChange(e.target.value)}
					placeholder="Name"
					required
					value={clusterName}
				/>
			</Option>

			<Option header="Versions">
				<VersionSelector onChange={e => onClusterVersionChange(e as string)} selectedVersion={clusterVersion} versions={versions.data} />
			</Option>

			<Option header="Loader">
				{/* TODO: fixme */}
				<Dropdown defaultSelectedKey="vanilla" onSelectionChange={e => onClusterLoaderChange(e.toString() as GameLoader)} selectedKey={clusterLoader}>
					{LOADERS.map(loader => (
						<Dropdown.Item id={loader} key={loader}>
							<div className="flex flex-row gap-2">
								<LoaderIcon className="size-5" loader={loader as GameLoader} />
								{upperFirst(loader)}
							</div>
						</Dropdown.Item>
					))}
				</Dropdown>
			</Option>
		</div>
	);
}

interface VersionSelectorProps {
	versions: Array<Version> | undefined;
	selectedVersion: string | Array<string>;
	onChange: (value: string | Array<string>) => void;
}

interface VersionReleaseFilters {
	old_alpha: boolean;
	old_beta: boolean;
	release: boolean;
	snapshot: boolean;
}

const DEFAULT_FILTERS: VersionReleaseFilters = {
	old_alpha: false,
	old_beta: false,
	release: true,
	snapshot: false,
};

function VersionSelector({ versions, selectedVersion, onChange }: VersionSelectorProps) {
	const [filters, setFilters] = useState<VersionReleaseFilters>(DEFAULT_FILTERS);

	const filteredVersions = useMemo(() => {
		if (!versions)
			return [];

		if (Object.values(filters).every(value => !value))
			return versions;

		return versions.filter(version => filters[version.type]);
	}, [versions, filters]);

	const toggleFilter = useCallback((name: keyof VersionReleaseFilters) => {
		setFilters(prev => ({
			...prev,
			[name]: !prev[name],
		}));
	}, []);

	const filterEntries = useMemo(() =>
		Object.entries(filters) as Array<[keyof VersionReleaseFilters, boolean]>, [filters]);

	return (
		<div className="flex flex-1 flex-row gap-2">
			<SelectList
				className="min-h-40 min-w-3/5 max-h-40 border border-component-border bg-component-bg"
				onChange={onChange}
				value={selectedVersion}
			>
				<ScrollableContainer>
					{filteredVersions.map(version => (
						<SelectList.Item key={version.id} value={version.id}>
							{version.id}
						</SelectList.Item>
					))}
				</ScrollableContainer>
			</SelectList>

			<div className="flex flex-1 flex-col gap-y-2 overflow-hidden border border-component-border bg-component-bg rounded-lg p-2">
				{filterEntries.map(([filterName, isSelected]) => (
					<Checkbox
						aria-label={`Filter ${filterName.replace('_', ' ')}`}
						className="selected:bg-component-bg-pressed rounded-lg p-1 text-sm"
						isSelected={isSelected}
						key={filterName}
						onChange={() => toggleFilter(filterName)}
					>
						<span className="capitalize">{filterName.replace('_', ' ')}</span>
					</Checkbox>
				))}
			</div>
		</div>
	);
}

type OptionProps = {
	header: string;
	children?: JSX.Element | Array<JSX.Element>;
} & RefAttributes<HTMLDivElement>;

function Option({ header, children, ...divProps }: OptionProps) {
	return (
		<div {...divProps} className="flex flex-col gap-y-2 items-stretch">
			<h3 className="text-left text-md text-fg-secondary font-semibold uppercase">
				{header}
			</h3>
			{children}
		</div>
	);
}
