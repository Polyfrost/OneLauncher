import type { HTMLMotionProps, MotionValue } from 'motion/react';
import { Button } from '@onelauncher/common/components';
import { useCanGoBack, useRouter } from '@tanstack/react-router';
import { ArrowLeftIcon } from '@untitled-theme/icons-react';
import { motion, useScroll, useTransform } from 'motion/react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import React, { useCallback, useRef } from 'react';
import { twMerge } from 'tailwind-merge';

export function SheetPage({
	headerSmall,
	headerLarge,
	children,
	scrollContainerRef,
}: {
	headerSmall: React.ReactNode;
	headerLarge: React.ReactNode;
	children?: React.ReactNode;
	scrollContainerRef?: React.RefObject<HTMLElement | null>;
}) {
	const scrollContainer = useRef<HTMLElement>(null);
	const headerLargeRef = useRef<HTMLDivElement>(null);

	const setScrollContainerRef = useCallback((el: HTMLElement | null) => {
		scrollContainer.current = el;
		if (scrollContainerRef)
			scrollContainerRef.current = el;
	}, [scrollContainerRef]);

	const { scrollYProgress } = useScroll({
		axis: 'y',
		container: scrollContainer,
		target: headerLargeRef,
		layoutEffect: false,
		offset: ['end start', 'start start'],
	});

	const smallTop = useTransform(scrollYProgress, [0, 1], ['0', '100%']);
	const smallOpacity = useTransform(scrollYProgress, [0, 1], ['1', '0']);
	const largeOpacity = useTransform(scrollYProgress, [0, 1], ['0', '1']);

	return (
		<div className="h-full">
			<HeaderBackgrounds scrollYProgress={scrollYProgress} />

			<div className="flex flex-col w-full min-h-full h-0">
				<div className="sticky top-0 left-0 mx-12 flex flex-row gap-4 h-8 overflow-hidden">
					<GoBackButton />

					<motion.div
						className="relative w-full h-full left-0" style={{
							top: smallTop,
							opacity: smallOpacity,
						}}
					>
						{headerSmall}
					</motion.div>
				</div>

				<OverlayScrollbarsComponent className="flex flex-col flex-1 mask-t-from-97% pt-4 relative" ref={ref => setScrollContainerRef(ref?.osInstance()?.elements().content ?? null)}>
					<div className="flex flex-col mx-12 gap-4 flex-1 min-h-full">
						<motion.div className="w-full" ref={headerLargeRef} style={{ opacity: largeOpacity }}>
							{headerLarge}
						</motion.div>

						{children}
					</div>
				</OverlayScrollbarsComponent>
			</div>
		</div>
	);
}

SheetPage.Content = ({
	children,
	className,
	...rest
}: Omit<HTMLMotionProps<'div'>, 'animate' | 'initial'>) => (
	<motion.div
		animate={{
			bottom: 0,
			opacity: 1,
		}}
		className={twMerge('relative flex flex-col px-10 py-6 last:mb-8 h-full bg-page-elevated rounded-2xl', className)}
		exit={{
			opacity: 0,
		}}
		initial={{
			opacity: 0,
		}}
		transition={{ duration: 0.25 }}
		{...rest}
	>
		{children}
	</motion.div>
);

function GoBackButton() {
	const router = useRouter();
	const canGoBack = useCanGoBack();

	if (!canGoBack)
		return undefined;

	const onClick = () => {
		router.history.back();
	};

	return (
		<Button color="ghost" onPress={onClick}>
			<ArrowLeftIcon height={20} />
			<span className="h-5">Back</span>
		</Button>
	);
}

function HeaderBackgrounds({
	scrollYProgress,
}: {
	scrollYProgress: MotionValue<number>;
}) {
	const top = useTransform(scrollYProgress, [1, 0], ['0px', '-60px']);

	return (
		<motion.div
			className="absolute left-0 h-full w-full overflow-hidden"
			style={{
				top,
			}}
		>
			<div
				className="absolute -z-10 -top-20 h-50 left-0 w-screen" style={{
					background: 'rgba(0, 0, 0, 0.20)',
				}}
			>
			</div>

			<div
				className="relative h-50 top-0 left-0 w-screen"
				style={{
					background: 'linear-gradient(180deg, rgba(17, 23, 28, 0.00) 0%, #11171C 60.1%)',
				}}
			>
			</div>

			<div className="relative top-0 left-0 w-full h-full bg-page"></div>
		</motion.div>
	);
}
