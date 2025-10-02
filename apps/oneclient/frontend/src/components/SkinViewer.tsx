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
	playerRotatePhi?: number;
	playerRotateTheta?: number;
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
	elytra?: boolean;
}

const defaultIdleAnimation = new skinviewer.IdleAnimation();

export function SkinViewer({
	skinUrl,
	capeUrl,
	width = 260,
	height = 300,
	className,
	autoRotate = true,
	autoRotateSpeed = 0.25,
	showText = true,
	playerRotatePhi = Math.PI / 3,
	playerRotateTheta = -Math.PI / 6,
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
	elytra = false,
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

		const setAngle = (phi: number, theta: number) => {
			const r = viewer.controls.object.position.distanceTo(viewer.controls.target);
			const x = r * Math.cos(phi - Math.PI / 2) * Math.sin(theta) + viewer.controls.target.x;
			const y = r * Math.sin(phi + Math.PI / 2) + viewer.controls.target.y;
			const z = r * Math.cos(phi - Math.PI / 2) * Math.cos(theta) + viewer.controls.target.z;
			viewer.controls.object.position.set(x, y, z);
			viewer.controls.object.lookAt(viewer.controls.target);
		};
		setAngle(playerRotatePhi, playerRotateTheta);

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
		if (!viewerRef.current)
			return;

		if (capeUrl)
			viewerRef.current.loadCape(capeUrl, { backEquipment: elytra ? 'elytra' : 'cape' });
		else
			viewerRef.current.resetCape();
	}, [capeUrl, elytra]);

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
