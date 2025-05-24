import type { JSX } from "react";

export type SettingsRowProps = {
    title: JSX.Element | string;
    description: JSX.Element | string;
    icon: JSX.Element;
    disabled?: boolean;
    children?: JSX.Element;
};

function SettingsRow(props: SettingsRowProps) {
    return (
        <div
            className={`
                flex flex-row items-center gap-3.5 rounded-xl p-4
                ${props.disabled ? 'bg-component-bg-disabled' : 'bg-page-elevated hover:bg-component-bg-hover'}
            `}
        >
            <div className="flex h-8 w-8 items-center justify-center">
                {props.icon}
            </div>

            <div className="flex flex-1 flex-col gap-2">
                <h3 className="text-lg capitalize leading-tight">{props.title}</h3>
                <p className="text-sm text-fg-secondary">{props.description}</p>
            </div>

            <div className="flex h-9 flex-row items-center gap-2">
                {props.children}
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
