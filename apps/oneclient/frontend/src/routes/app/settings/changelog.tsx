import { bindings } from '@/main';
import { Sidebar } from '@/routes/app/settings/route';
import { useCommand } from '@onelauncher/common';
import { createFileRoute } from '@tanstack/react-router';
import { ChevronDownIcon } from '@untitled-theme/icons-react';
import { useEffect, useState } from 'react';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/app/settings/changelog')({
	component: RouteComponent,
});

interface VersionDropdownProps {
	version: string;
	changes: Array<string>;
	active?: boolean;
	current?: boolean;
}

function VersionDropdown({ version, changes, active, current }: VersionDropdownProps) {
	const [isOpen, setIsOpen] = useState<boolean>(current ?? active ?? false);

	return (
		<div className={twMerge('bg-page-elevated px-4 rounded-lg mb-4', isOpen ? 'py-4' : 'py-2 pt-4')}>
			<div className="flex flex-row justify-between" onClick={() => setIsOpen(prev => !prev)}>
				<h1 className="text-xl font-semibold mb-2 cursor-pointer">{version}{current ? ' (Currently Installed)' : ''} </h1>
				<ChevronDownIcon className={twMerge((isOpen ? '' : 'rotate-90'), 'duration-500')} />
			</div>

			<div
				className={twMerge(isOpen ? 'opacity-100 h-auto' : 'opacity-0 h-0 overflow-hidden', 'transition-all duration-300 ease-out')}
				style={{ transitionProperty: 'opacity, height' }}
			>
				{isOpen && (
					<div>
						{changes.length > 0
							? (
									<ul className="list-disc pl-6">
										{changes.map(change => (
											<li className="px-1 text-fg-secondary" key={change}>
												{change}
											</li>
										))}
									</ul>
								)
							: <p>No changes recorded for this version.</p>}
					</div>
				)}
			</div>
		</div>
	);
};

interface ChangelogGroup {
	version: string;
	changes: Array<string>;
}

function RouteComponent() {
	const [data, setData] = useState<string>('');
	const [isLoading, setIsLoading] = useState<boolean>(true);
	const [error, setError] = useState<any | null>(null);
	const { data: version } = useCommand(['getPackageVersion'], () => bindings.debug.getPackageVersion());

	useEffect(() => {
		const getData = async () => {
			try {
				const response = await fetch('https://raw.githubusercontent.com/Polyfrost/DataStorage/refs/heads/main/oneclient/CHANGE_LOG.md');
				if (!response.ok)
					throw new Error('Failed to fetch changelog');

				const data = await response.text();
				setData(data);
			}
			catch (error: unknown) {
				if (error instanceof Error)
					setError(error.message);
				else
					setError('An unknown error occurred');
			}
			finally {
				setIsLoading(false);
			}
		};

		getData();
	}, []);

	if (isLoading)
		return <div>Loading...</div>;

	if (error)
		return <div>Error: {error}</div>;

	const changelogGroups = data.split('\n').reduce<Array<ChangelogGroup>>((acc, line) => {
		if (line.startsWith('# ')) {
			acc.push({ version: line.replaceAll('# ', '').trim(), changes: [] });
		}
		else if (line.startsWith('- ')) {
			if (acc.length > 0)
				acc[acc.length - 1].changes.push(line.slice(2).trim());
		}
		else if (line === '###') {
			return acc;
		}
		return acc;
	}, []);

	return (
		<Sidebar.Page>
			<div className="h-full">
				{changelogGroups.map((group, groupIndex) => (
					group.version
						? (
								<VersionDropdown
									active={groupIndex === 0 ? true : undefined}
									changes={group.changes}
									current={group.version === version ? true : undefined}
									key={group.version}
									version={group.version}
								/>
							)
						: null
				))}
			</div>
		</Sidebar.Page>
	);
}
