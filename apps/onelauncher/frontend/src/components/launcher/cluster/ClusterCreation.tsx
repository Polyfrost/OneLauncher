import type { JSX } from 'react';
import DefaultBanner from '@/assets/images/default_banner.png';
import LauncherIcon from '@/components/content/LauncherIcon';
import Modal from '@/components/overlay/Modal';
import { bindings } from '@/main';
import { LAUNCHER_IMPORT_TYPES } from '@/utils';
import { useCommand } from '@onelauncher/common';
import { Button, Dropdown, TextField } from '@onelauncher/common/components';
import { Server01Icon, User03Icon } from '@untitled-theme/icons-react';
import { useCallback, useState } from 'react';

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

	const handleNext = useCallback(async () => {
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
	const nextButtonText = isLastInputStep ? 'Create Cluster' : 'Next';

	let isNextDisabled = false;
	if (currentStepConfig.id === 'loader' && !formData.loader)
		isNextDisabled = true;
	else if (
		currentStepConfig.id === 'info'
		&& (!formData.clusterName.trim() || !formData.clusterVersion.trim())
	)
		isNextDisabled = true;

	return (
		<>
			<Modal.Trigger>
				<Button
					children="New Cluster"
				/>

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
									<h1 className="h-10 text-fg-primary -mt-2">New Cluster</h1>
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
									children="Back"
									color="secondary"
									isDisabled={isFirstStep}
									onClick={handleBack}
								/>
								<Button
									children={nextButtonText}
									color="primary"
									isDisabled={isNextDisabled}
									onClick={handleNext}
								/>
							</div>
						</div>
					</div>
				</Modal>
			</Modal.Trigger>
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
}

function ClusterInformation({
	clusterName,
	onClusterNameChange,
	clusterVersion,
	onClusterVersionChange,
}: ClusterInformationProps) {
	return (
		<div className="flex flex-col gap-y-4">
			<TextField
				className="w-full"
				onChange={e => onClusterNameChange(e.target.value)}
				placeholder="My Awesome Server"
				value={clusterName}
			/>
			<Dropdown placeholder="Select a version">
				<Dropdown.Item>Slmalr</Dropdown.Item>
				<Dropdown.Item>Slmalr 2</Dropdown.Item>
			</Dropdown>
		</div>
	);
}
