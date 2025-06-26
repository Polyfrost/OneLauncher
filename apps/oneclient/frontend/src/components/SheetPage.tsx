import type { MotionValue } from 'motion/react';
import useAppShellStore from '@/stores/appShellStore';
import { getLocationName } from '@/utils/locationMapping';
import { Button } from '@onelauncher/common/components';
import { Outlet, useNavigate } from '@tanstack/react-router';
import { ArrowLeftIcon } from '@untitled-theme/icons-react';
import { motion, useScroll, useTransform } from 'motion/react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import React, { useRef } from 'react';

export function SheetPage({
	headerSmall,
	headerLarge,
}: {
	headerSmall: React.ReactNode;
	headerLarge: React.ReactNode;
}) {
	const scrollContainer = useRef<HTMLElement>(null);
	const headerLargeRef = useRef<HTMLDivElement>(null);

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

				<OverlayScrollbarsComponent className="flex flex-col flex-1 mask-t-from-97% pt-4 relative" ref={ref => scrollContainer.current = ref?.osInstance()?.elements().content ?? null}>
					<div className="flex flex-col mx-12 gap-4">
						<motion.div className="w-full" ref={headerLargeRef} style={{ opacity: largeOpacity }}>
							{/* <headerLarge ref={headerLargeRef as React.RefObject<HTMLDivElement>} /> */}
							{headerLarge}
						</motion.div>

						<motion.div
							animate={{ bottom: 0 }}
							className="relative flex flex-col px-10 py-6 mb-8 bg-page-elevated rounded-2xl"
							initial={{ bottom: '-80px' }}
						>
							<Outlet />
						</motion.div>
					</div>
				</OverlayScrollbarsComponent>
			</div>
		</div>
	);
}

function GoBackButton() {
	const prevLocation = useAppShellStore(state => state.prevLocation);
	const navigate = useNavigate();

	const onClick = () => {
		if (!prevLocation)
			return;

		navigate({
			to: prevLocation.pathname,
			search: prevLocation.search,
			hash: prevLocation.hash,
			state: prevLocation.state,
		});
	};

	if (!prevLocation)
		return undefined;

	return (
		<Button color="ghost" onClick={onClick}>
			<ArrowLeftIcon height={20} />
			<span className="h-5">{getLocationName(prevLocation.pathname)}</span>
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
			className="absolute left-0 h-full w-full"
			style={{
				top,
			}}
		>
			<div
				className="absolute -z-10 -top-20 h-50 left-0 w-screen animate-fade animate-duration-200" style={{
					background: 'rgba(0, 0, 0, 0.20)',
				}}
			>
			</div>

			<div
				className="relative h-50 top-0 left-0 w-screen animate-fade animate-duration-200"
				style={{
					background: 'linear-gradient(180deg, rgba(17, 23, 28, 0.00) 0%, #11171C 60.1%)',
				}}
			>
			</div>

			<div className="relative top-0 left-0 w-full h-full bg-page animate-fade animate-duration-200"></div>
		</motion.div>
	);
}
