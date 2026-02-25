import type { HTMLAttributes, ReactNode, Ref } from 'react';
import { createContext, useCallback, useContext, useMemo, useState } from 'react';
import { twMerge } from 'tailwind-merge';

// i thank chatgpt for this i dont want to replace current tabs component yet so this is going to be here for some time
interface TabsContextType {
	activeTab: string;
	setActiveTab: (value: string) => void;
}

const TabsContext = createContext<TabsContextType | null>(null);

function useTabs() {
	const context = useContext(TabsContext);
	if (!context)
		throw new Error('useTabs must be used within a <Tabs> component');

	return context;
}

interface TabsProps {
	defaultValue: string;
	onTabChange?: (value: string) => void;
	children: React.ReactNode;
}

export function Tabs({ defaultValue, onTabChange, children }: TabsProps) {
	const [activeTab, setActiveTabInternal] = useState(defaultValue);
	const setActiveTab = useCallback((value: string) => {
		setActiveTabInternal(value);
		if (onTabChange)
			onTabChange(value);
	}, [onTabChange]);

	const contextValue = useMemo(() => ({ activeTab, setActiveTab }), [activeTab, setActiveTab]);

	return (
		<TabsContext.Provider value={contextValue}>
			{children}
		</TabsContext.Provider>
	);
}
interface TabListProps extends HTMLAttributes<HTMLDivElement> {
	floating?: boolean;
	height?: boolean;
	ref?: Ref<HTMLDivElement>;
}

export function TabList({
	className,
	children,
	floating = false,
	height = false,
	ref,
}: TabListProps) {
	return (
		<div className={twMerge('pointer-events-none sticky top-0 z-10 w-full', height ? 'min-h-[74px] h-[74px] max-h-[74px]' : '')} ref={ref}>
			<div
				className={twMerge(
					'pointer-events-auto flex flex-row gap-2 border border-transparent bg-page-elevated transition-all',
					floating
						? 'px-6 mx-4 py-3 shadow-lg border-ghost-overlay rounded-xl'
						: 'px-10 py-6 rounded-2xl',
					className,
				)}
				role="tablist"
			>
				{children}
			</div>
		</div>
	);
}

interface TabProps extends Omit<HTMLAttributes<HTMLButtonElement>, 'value'> {
	value: string;
}

export function Tab({
	children,
	value,
	className,
	...rest
}: TabProps) {
	const { activeTab, setActiveTab } = useTabs();
	const isActive = activeTab === value;

	return (
		<div className="relative flex justify-center items-center">
			<button
				aria-selected={isActive}
				className={twMerge(
					'text-center text-lg transition-all duration-100 after:duration-100 after:transition-all',
					isActive
						? 'text-fg-primary font-semibold partial-underline-75% pointer-events-none'
						: 'text-fg-secondary font-normal partial-underline-0% hover:partial-underline-60% hover:text-fg-secondary-hover pointer-events-auto',
					className,
				)}
				onClick={() => setActiveTab(value)}
				role="tab"
				type="button"
				{...rest}
			>
				{children}
			</button>
		</div>
	);
}

interface TabPanelProps extends HTMLAttributes<HTMLDivElement> {
	value: string;
}

export function TabPanel({ children, value, className, ...rest }: TabPanelProps) {
	const { activeTab } = useTabs();

	if (activeTab !== value)
		return null;

	return (
		<div
			className={twMerge('bg-page-elevated px-3 pt-3 w-full rounded-2xl h-full', className)}
			role="tabpanel"
			{...rest}
		>
			{children}
		</div>
	);
}

export function TabContent({ children, className, ...rest }: HTMLAttributes<HTMLDivElement>) {
	return (
		<div className={twMerge('pt-4', className)} {...rest}>
			{children}
		</div>
	);
}
