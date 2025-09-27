import { getSkinUrl } from '@/utils/minecraft';
import { useEffect, useRef } from 'react';
import * as skinviewer from 'skinview3d';
import { twMerge } from 'tailwind-merge';

export interface SkinViewerProps {
	skinUrl?: string | undefined | null;
	capeUrl?: string | undefined | null;
	width?: number;
	height?: number;
	className?: string | undefined;
	autoRotate?: boolean;
	autoRotateSpeed?: number;
	showText?: boolean;
	playerRotateX?: number;
	playerRotateY?: number;
	playerRotateZ?: number;
	translateRotateX?: number;
	translateRotateY?: number;
	translateRotateZ?: number;
	zoom?: number;
	animate?: boolean;
	animation?: skinviewer.PlayerAnimation;
	enableDamping?: boolean;
	enableZoom?: boolean;
	enableRotate?: boolean;
	enablePan?: boolean;
}


const defaultIdleAnimation = new skinviewer.IdleAnimation();

export function SkinViewer({
	skinUrl,
	capeUrl,
	width = 260,
	height = 300,
	className,
	autoRotate = false,
	autoRotateSpeed = 0.25,
	showText = true,
	playerRotateX = 0,
	playerRotateY = 0,
	playerRotateZ = 0,
	translateRotateX = 0,
	translateRotateY = 0,
	translateRotateZ = 0,
	zoom = 0.9,
	animate = false,
	animation = defaultIdleAnimation,
	enableDamping = true,
	enableZoom = true,
	enableRotate = true,
	enablePan = true,
}: SkinViewerProps) {
	const canvasRef = useRef<HTMLCanvasElement>(null);
	const viewerRef = useRef<skinviewer.SkinViewer | null>(null);

	useEffect(() => {
		if (!canvasRef.current)
			return;

		const viewer = new skinviewer.SkinViewer({
			canvas: canvasRef.current,
		});

		viewer.controls.enableDamping = enableDamping;
		viewer.controls.enableZoom = enableZoom;
		viewer.controls.enableRotate = enableRotate;
		viewer.controls.enablePan = enablePan;

		viewer.zoom = zoom;
		viewer.playerWrapper.rotateX(playerRotateX);
		viewer.playerWrapper.rotateY(playerRotateY);
		viewer.playerWrapper.rotateZ(playerRotateZ);
		viewer.playerWrapper.translateX(translateRotateX);
		viewer.playerWrapper.translateY(translateRotateY);
		viewer.playerWrapper.translateZ(translateRotateZ);

		viewer.animation = animation;

		viewer.autoRotate = autoRotate;
		viewer.autoRotateSpeed = autoRotateSpeed;

		viewerRef.current = viewer;

		return () => {
			viewer.dispose();
		};
	}, []);

	useEffect(() => {
		if (!viewerRef.current)
			return;

		viewerRef.current.loadSkin(getSkinUrl(skinUrl));
	}, [skinUrl]);

	useEffect(() => {

	}, [capeUrl]);

	useEffect(() => {
		if (!viewerRef.current)
			return;

		if (capeUrl)
			viewerRef.current.loadCape(capeUrl);
		else
			viewerRef.current.resetCape();
	}, [capeUrl]);

	useEffect(() => {
		if (!viewerRef.current)
			return;

		viewerRef.current.setSize(width, height);
	}, [width, height]);

	useEffect(() => {
		if (!viewerRef.current)
			return;

		viewerRef.current.animation = animation;
	}, [animation]);

	useEffect(() => {
		if (!viewerRef.current || !viewerRef.current.animation)
			return;

		viewerRef.current.animation.paused = !animate;
	}, [animate]);

	return (
		<div className={twMerge('flex flex-col justify-center items-center', className)} style={{ minWidth: `${width}px`, minHeight: `${height}px` }}>
			<canvas
				height={height}
				ref={canvasRef}
				width={width}
			/>

			{showText ? <span className="text-fg-secondary text-xs">Hold to drag. Scroll to zoom in/out.</span> : <></>}
		</div>
	);
}
