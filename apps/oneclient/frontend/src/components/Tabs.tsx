import type { LinkProps } from '@tanstack/react-router';
import { Link } from '@tanstack/react-router';
import { twMerge } from 'tailwind-merge';

export function TabList({
	className,
	children,
	floating = false,
	ref,
}: React.HTMLAttributes<HTMLDivElement> & {
	floating?: boolean;
	ref?: React.Ref<HTMLDivElement>;
}) {
	return (
		<div className={twMerge('flex flex-row gap-2 px-10 border border-transparent bg-page-elevated transition-all', floating ? 'py-3 shadow-lg border-ghost-overlay rounded-xl' : 'py-6 rounded-2xl', className)} ref={ref}>
			{children}
		</div>
	);
}

export function Tab({
	to,
	children,
}: Omit<LinkProps, 'activeOptions' | 'activeProps' | 'inactiveProps' | 'className'>) {
	return (
		<div className="relative flex justify-center items-center">
			<Link
				activeOptions={{
					exact: true,
				}}
				activeProps={{
					className: 'text-fg-primary font-semibold partial-underline-75% pointer-events-none',
				}}
				className="text-center text-lg transition-all duration-100 after:duration-100 after:transition-all"
				inactiveProps={{
					className: 'text-fg-secondary font-normal partial-underline-0% hover:partial-underline-60% hover:text-fg-secondary-hover after:text-fg-secondary-hover pointer-events-auto',
				}}
				to={to}
			>
				{children}
			</Link>
		</div>
	);
}
