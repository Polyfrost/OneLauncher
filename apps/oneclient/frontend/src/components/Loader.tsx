import type { HTMLAttributes, ReactNode } from 'react';
import type { SpinnerProps } from './Spinner';
import { motion } from 'motion/react';
import { Suspense } from 'react';
import { twMerge } from 'tailwind-merge';
import { Spinner } from './Spinner';

interface LoaderProps extends HTMLAttributes<HTMLDivElement> {
	children?: ReactNode;
	spinner?: SpinnerProps;
}

export function LoaderContainer({
	loading = false,
	children,
	spinner,
	className,
}: LoaderProps & {
	loading?: boolean;
}) {
	return loading ? <Fallback className={className} spinner={spinner} /> : <LoadedChildren className={className}>{children}</LoadedChildren>;
}

export function LoaderSuspense({
	children,
	spinner,
	className,
}: LoaderProps) {
	return (
		<Suspense fallback={<Fallback className={className} spinner={spinner} />}>
			<LoadedChildren className={className}>
				{children}
			</LoadedChildren>
		</Suspense>
	);
}

function Fallback({
	spinner,
	className,
}: {
	spinner?: SpinnerProps;
	className?: string;
}) {
	return (
		<div className={twMerge('w-full h-full flex justify-center items-center', className)}>
			<Spinner {...spinner} />
		</div>
	);
}

function LoadedChildren({
	children,
	className,
}: {
	children: ReactNode;
	className?: string;
}) {
	return (
		<motion.div
			animate={{ opacity: 1 }}
			className={twMerge('w-full h-full', className)}
			initial={{ opacity: 0 }}
		>
			{children}
		</motion.div>
	);
}
