import type { JSX } from 'react';

export interface SettingsRowProps {
	title: JSX.Element | string;
	description: JSX.Element | string;
	icon: JSX.Element;
	disabled?: boolean;
	children?: JSX.Element;
}

export function SettingsRow({ disabled, description, icon, title, children }: SettingsRowProps) {
	return (
		<div
			className={`
                flex flex-row items-center gap-3.5 rounded-xl p-4 my-2
                ${disabled ? 'bg-component-bg-disabled' : 'bg-page-elevated hover:bg-component-bg-hover'}
            `}
		>
			<div className="flex h-8 w-8 items-center justify-center">
				{icon}
			</div>

			<div className="flex flex-1 flex-col gap-2">
				<p className="text-lg capitalize leading-tight">{title}</p>
				<p className="text-sm text-fg-secondary">{description}</p>
			</div>

			<div className="flex h-9 flex-row items-center gap-2">
				{children}
			</div>
		</div>
	);
}

interface SettingsRowHeaderProps {
	className?: string;
	children?: React.ReactNode;
}

SettingsRow.Header = ({ className, children }: SettingsRowHeaderProps) => {
	const headerClasses = `mt-4 mb-1 ml-2 text-md text-fg-secondary uppercase ${className || ''}`;
	return <h3 className={headerClasses}>{children}</h3>;
};

export default SettingsRow;
