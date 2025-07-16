/* eslint-disable eslint-comments/require-description -- i trust that one github discussion user on this */
/* eslint-disable react/no-unstable-default-props */

// Taken from https://github.com/TanStack/router/discussions/823#discussioncomment-12765059
// Thank you https://github.com/danielytics

import type { AnyRoute } from '@tanstack/react-router';
import type { MotionProps } from 'motion/react';
import { Outlet, useMatch, useRouter } from '@tanstack/react-router';
import { motion } from 'motion/react';
import { createContext, useContext, useEffect, useRef, useState } from 'react';

interface TransitionProps {
	initial?: MotionProps['initial'];
	animate?: MotionProps['animate'];
}

interface AnimatedOutletProps {
	enter?: TransitionProps;
	exit?: TransitionProps;
	transition?: MotionProps['transition'];
	from: AnyRoute['id'];
	clone?: boolean;
	children?: (children: React.ReactNode) => React.ReactNode;
}

interface AnimatedOutletWrapperProps {
	children: React.ReactNode;
}

type TakeSnapshotFn = () => void;

type Registry = Map<string, TakeSnapshotFn>;

const AnimatedOutletContext = createContext<Registry>(new Map());

function isDescendant(pathname: string, destinationPath: string) {
	return pathname === '/'
		|| (destinationPath.startsWith(pathname)
			&& (destinationPath.length === pathname.length
				|| destinationPath.charAt(pathname.length) === '/'));
}

export function AnimatedOutletProvider({ children }: AnimatedOutletWrapperProps) {
	const router = useRouter();
	const registry = useRef<Registry>(new Map());

	useEffect(() => {
		// NOTE: This should be onBeforeNavigate, but due to https://github.com/TanStack/router/issues/3920 it's not working.
		// For now, we use onBeforeLoad, which runs right after onBeforeNavigate.
		// See: https://github.com/TanStack/router/blob/f8015e7629307499d4d6245077ad84145b6064a7/packages/router-core/src/router.ts#L2027
		const unsubscribe = router.subscribe('onBeforeLoad', ({ toLocation, pathChanged }) => {
			if (pathChanged) {
				const destinationPath = toLocation.pathname;
				// Find the outlet with the longest pathname, that is part of the destination route
				let takeSnapshot: TakeSnapshotFn | null = null;
				let longestLength = 0;
				for (const [pathname, snapshotFn] of registry.current.entries())
					if (isDescendant(pathname, destinationPath) && pathname.length > longestLength) {
						longestLength = pathname.length;
						takeSnapshot = snapshotFn;
					}

				if (takeSnapshot)
				// Take a snapshot of the deepest outlet
					takeSnapshot();
			}
		});

		return unsubscribe;
	}, [router]);

	return (
		<AnimatedOutletContext value={registry.current}>
			{children}
		</AnimatedOutletContext>
	);
}

export function AnimatedOutlet({
	enter,
	exit,
	transition = { duration: 0.3 },
	from,
	clone = true,
	children,
}: AnimatedOutletProps) {
	const [snapshots, setSnapshots] = useState<Array<{ node: HTMLElement; id: number }>>([]);
	const [pathname, setPathname] = useState<string | null>(null);
	const outletRef = useRef<HTMLDivElement>(null);
	const nextId = useRef(0);

	const registry = useContext(AnimatedOutletContext);

	useEffect(() => {
		if (!pathname)
			return;

		registry.set(pathname, () => {
			const outletNode = outletRef.current!;
			const snapshotNode = outletNode.firstChild as HTMLElement | null;
			if (snapshotNode) {
				let node = snapshotNode;
				if (clone)
					node = snapshotNode.cloneNode(true) as HTMLElement;
				else
					snapshotNode.remove();

				const newSnapshot = { node, id: nextId.current++ };
				setSnapshots(prevSnapshots => [...prevSnapshots, newSnapshot]);
			}
		});

		return () => {
			registry.delete(pathname);
		};
	}, [registry, pathname, clone]);

	const handleAnimationComplete = (id: number) => {
		setSnapshots(prevSnapshots =>
			prevSnapshots.filter(snapshot => snapshot.id !== id));
	};

	return (
		<>
			{pathname === null && <GetPathName from={from} setPathname={setPathname} />}
			<div className="relative w-full h-full">
				{snapshots.map(snapshot => (
					<motion.div
						animate={exit?.animate || { opacity: 1 }}
						aria-hidden="true"
						className="absolute inset-0 pointer-events-none w-full h-full"
						initial={exit?.initial || { opacity: 1 }}
						key={snapshot.id}
						onAnimationComplete={() => handleAnimationComplete(snapshot.id)}
						ref={(el) => {
							if (el)
								el.append(snapshot.node);
						}}
						transition={transition}
					/>
				))}

				<motion.div
					animate={enter?.animate}
					className="relative w-full h-full"
					initial={enter?.initial}
					key={nextId.current}
					ref={outletRef}
					transition={transition}
				>
					<div className="w-full h-full">
						{children?.(<Outlet />) ?? <Outlet />}
					</div>
				</motion.div>
			</div>
		</>
	);
}

function GetPathName({ from, setPathname }: { from: AnyRoute['id']; setPathname: (pathname: string) => void }) {
	const match = useMatch({ from });

	useEffect(() => {
		setPathname(match.pathname);
	}, [match.pathname, setPathname]);

	return null;
}
