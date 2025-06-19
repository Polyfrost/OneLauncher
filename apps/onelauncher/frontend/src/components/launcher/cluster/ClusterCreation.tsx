import type { GameLoader, IngressPayload, Version, VersionType } from '@/bindings.gen';
import type { JSX, RefAttributes } from 'react';
import DefaultBanner from '@/assets/images/default_banner.png';
import DefaultClusterBanner from '@/assets/images/default_instance_cover.jpg';
import LauncherIcon from '@/components/content/LauncherIcon';
import Modal from '@/components/overlay/Modal';
import ScrollableContainer from '@/components/ScrollableContainer';
import { bindings } from '@/main';
import { LAUNCHER_IMPORT_TYPES, LOADERS, upperFirst } from '@/utils';
import { useCommand } from '@onelauncher/common';
import { Button, Dropdown, SelectList, TextField } from '@onelauncher/common/components';
import { useQueryClient } from '@tanstack/react-query';
import { ArrowRightIcon, PlusIcon, SearchMdIcon, Server01Icon, TextInputIcon, User03Icon } from '@untitled-theme/icons-react';
import { useCallback, useEffect, useState } from 'react';
import { Checkbox } from 'react-aria-components';
import LoaderIcon from '../LoaderIcon';

const STEPS = [
	{
		id: 'loader',
		title: 'Select Provider',
	},
	{
		id: 'info',
		title: 'Game Setup',
	},
] as const;

export function NewClusterCreate() {
	const [currentStepIndex, setCurrentStepIndex] = useState(0);
	const [formData, setFormData] = useState({
		loader: '',
		clusterName: '',
		clusterVersion: '',
	});

	const create = useCommand('createCluster', () => bindings.core.createCluster({
		icon: DefaultClusterBanner,
		mc_loader: formData.loader as GameLoader,
		mc_loader_version: null,
		mc_version: formData.clusterVersion,
		name: formData.clusterName,
	}), {
		enabled: false,
		subscribed: false,
	});

	const queryClient = useQueryClient();

	function handleCreate() {
		if (!isLastInputStep)
			return;

		create.refetch();

		if (create.isError)
			console.error(create.error.message);

		queryClient.invalidateQueries({ queryKey: ['getClusters'], exact: true });
	}

	const handleNext = useCallback(async () => {
		if (currentStepIndex === STEPS.length - 1)
			handleCreate();

		if (currentStepIndex < STEPS.length - 1)
			setCurrentStepIndex(currentStepIndex + 1);
	}, [currentStepIndex]);

	const handleBack = useCallback(() => {
		if (currentStepIndex > 0)
			setCurrentStepIndex(currentStepIndex - 1);
	}, [currentStepIndex]);

	const currentStepConfig = STEPS[currentStepIndex];
	let stepContent: JSX.Element;

	switch (currentStepConfig.id) {
		case 'loader': {
			stepContent = (
				<ClusterLoader
					onSelectLoader={loader => setFormData(prev => ({ ...prev, loader }))}
					selectedLoader={formData.loader}
				/>
			);
			break;
		}

		case 'info': {
			stepContent = (
				<ClusterInformation
					clusterName={formData.clusterName}
					clusterVersion={formData.clusterVersion}
					onClusterLoaderChange={loader => setFormData(prev => ({ ...prev, loader }))}
					onClusterNameChange={name => setFormData(prev => ({ ...prev, clusterName: name }))}
					onClusterVersionChange={version =>
						setFormData(prev => ({ ...prev, clusterVersion: version }))}
				/>
			);
			break;
		}
	}

	const isFirstStep = currentStepIndex === 0;
	const isLastInputStep = currentStepIndex === STEPS.length - 1;
	const nextButtonText = isLastInputStep ? 'Create' : 'Next';

	let isNextDisabled = false;
	if (currentStepConfig.id === 'loader' && !formData.loader)
		isNextDisabled = true;
	else if (
		currentStepConfig.id === 'info'
		&& (!formData.clusterName.trim() || !formData.clusterVersion.trim())
	)
		isNextDisabled = false;

	return (
		<>
			<div className="flex flex-row justify-between w-full">
				<div>
					<TextField className="p-1" iconLeft={<SearchMdIcon className="size-4" />} placeholder="Search for something..." />
				</div>

				<Modal.Trigger>
					<Button>
						<PlusIcon className="stroke-width-[2.2] size-5" />
						New cluster
					</Button>

					<Modal>
						<div className="min-w-sm flex flex-col rounded-lg bg-page text-center">
							<div className="theme-OneLauncher-Dark relative h-25 flex">
								<div className="absolute left-0 top-0 h-full w-full">
									<img alt="Header Image" className="h-full w-full rounded-t-lg" src={DefaultBanner} />
								</div>
								<div
									className="absolute left-0 top-0 h-full flex w-full flex-row items-center justify-start gap-x-4 bg-[radial-gradient(at_center,#00000077,transparent)] px-10"
								>
									<Server01Icon className="h-8 w-8 text-fg-primary" />
									<div className="flex flex-col items-start justify-center">
										<h1 className="h-10 text-fg-primary text-2xl font-semibold">New Cluster</h1>
										<span className="text-fg-primary">{STEPS[currentStepIndex].title}</span>
									</div>
								</div>
							</div>
							<div className="flex flex-col rounded-b-lg border border-white/5">
								<div className="p-3">
									{stepContent}
								</div>

								<div className="flex flex-row justify-end gap-x-2 p-3 pt-0">
									<Button
										children="Previous"
										color="ghost"
										isDisabled={isFirstStep}
										onClick={handleBack}
									/>
									<Button
										color="primary"
										isDisabled={isNextDisabled}
										onClick={handleNext}
									>
										{nextButtonText}
										{' '}
										<ArrowRightIcon />
									</Button>
								</div>
							</div>
						</div>
					</Modal>
				</Modal.Trigger>
			</div>
		</>
	);
}

interface ClusterLoaderProps {
	selectedLoader: string;
	onSelectLoader: (loader: string) => void;
}

function ClusterLoader({ selectedLoader, onSelectLoader }: ClusterLoaderProps) {
	return (
		<>
			<div className="grid grid-cols-3 gap-2">
				<ProviderCard
					icon={<User03Icon />}
					name="New"
					selected={selectedLoader === 'new'}
					setSelected={() => onSelectLoader('new')}
				/>

				{LAUNCHER_IMPORT_TYPES.map((data, i) => (
					<ProviderCard
						icon={<LauncherIcon launcher={data} />}
						key={i}
						name={data}
						selected={selectedLoader === data}
						setSelected={() => onSelectLoader(data)}
					/>
				))}
			</div>
		</>
	);
}

interface ProviderCardProps {
	setSelected: () => void;
	selected: boolean;
	icon: JSX.Element;
	name: string;
}

function ProviderCard(props: ProviderCardProps) {
	const { selected, setSelected, icon, name } = props;

	return (
		<div
			className={`flex flex-col justify-center items-center gap-y-3 py-2 px-4 hover:bg-component-bg-hover active:bg-component-bg-pressed rounded-lg ${selected ? 'bg-component-bg' : ''}`}
			onClick={() => setSelected()}
		>
			<div className="h-8 w-8 flex items-center justify-center [&>svg]:(w-8 h-8!)">
				{icon}
			</div>
			<span>{name}</span>
		</div>
	);
}

interface ClusterInformationProps {
	clusterName: string;
	onClusterNameChange: (name: string) => void;
	clusterVersion: string;
	onClusterVersionChange: (version: string) => void;
	onClusterLoaderChange: (loader: GameLoader) => void;
}

function ClusterInformation({
	clusterName,
	onClusterNameChange,
	clusterVersion,
	onClusterVersionChange,
	onClusterLoaderChange,
}: ClusterInformationProps) {
	const versions = useCommand('getGameVersions', bindings.core.getGameVersions);

	return (
		<div className="flex flex-col gap-y-4">
			<Option header="Name">
				<TextField
					className="w-full"
					iconLeft={<TextInputIcon className="size-4" />}
					onChange={e => onClusterNameChange(e.target.value)}
					placeholder="Name"
					value={clusterName}
				/>
			</Option>

			<Option header="Versions">
				<VersionSelector onChange={e => onClusterVersionChange(e as string)} selectedVersion={clusterVersion} versions={versions.data} />
			</Option>

			<Option header="Loader">
				{/* TODO: fixme */}
				<Dropdown defaultSelectedKey="vanilla" onSelectionChange={e => onClusterLoaderChange(e)}>
					{LOADERS.map(loader => (
						<Dropdown.Item key={loader}>
							<div className="flex flex-row">
								<LoaderIcon className="size-5" loader={loader as GameLoader} />
								{' '}
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

function VersionSelector(props: VersionSelectorProps) {
	const { versions, selectedVersion, onChange } = props;
	const [filteredVersions, setFilteredVersions] = useState<Array<Version>>([]);
	const [filters, setFilters] = useState<VersionReleaseFilters>({
		old_alpha: false,
		old_beta: false,
		release: true,
		snapshot: false,
	});

	const refresh = useCallback((filters: VersionReleaseFilters, versions: Array<Version>) => {
		setFilteredVersions(() => {
			if (Object.values(filters).every(value => value === false))
				return versions;

			return versions.filter(version => filters[version.type]);
		});
	}, []);

	const toggleFilter = (name: keyof VersionReleaseFilters) => {
		setFilters(prev => ({
			...prev,
			[name]: !prev[name],
		}));
	};

	useEffect(() => {
		if (versions)
			refresh(filters, versions);
	}, [filters, versions, refresh]);

	useEffect(() => {
		if (versions)
			setFilteredVersions(versions);
	}, [versions]);

	return (
		<div className="flex flex-1 flex-row gap-2">
			<SelectList className="min-h-40 min-w-3/5 max-h-40 border border-component-border bg-component-bg" onChange={onChange} value={selectedVersion}>
				<ScrollableContainer>
					{filteredVersions.map(version => (
						<SelectList.Item key={version.id} value={version.id}>{version.id}</SelectList.Item>
					))}
				</ScrollableContainer>
			</SelectList>

			<div className="flex flex-1 flex-col gap-y-2 overflow-hidden border border-component-border bg-component-bg rounded-lg p-2">
				{Object.keys(filters).map(filter => (
					<Checkbox
						className="selected:bg-component-bg-pressed rounded-lg"
						isSelected={filters[filter as VersionType]}
						key={filter}
						onChange={() => toggleFilter(filter as VersionType)}
					>
						{filter}
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

function Option(props: OptionProps) {
	return (
		<div {...props} className="flex flex-col gap-y-2 items-stretch">
			<h3 className="text-left text-md text-fg-secondary font-semibold uppercase">{props.header}</h3>
			{props.children}
		</div>
	);
}
