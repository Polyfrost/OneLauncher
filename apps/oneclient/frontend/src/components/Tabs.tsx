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
		<div className="flex justify-stretch items-center sticky top-0 z-10 min-h-[74px] h-[74px] max-h-[74px] w-full" ref={ref}>
			<div
				className={twMerge(
					'flex-1 flex flex-row gap-2 border border-transparent bg-page-elevated transition-all',
					floating
						? 'px-6 mx-4 py-3 shadow-lg border-ghost-overlay rounded-xl'
						: 'px-10 py-6 rounded-2xl',
					className,
				)}
			>
				{children}
			</div>
		</div>
	);
}

export function Tab({
	to,
	children,
	...rest
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
				{...rest}
			>
				{children}
			</Link>
		</div>
	);
}
