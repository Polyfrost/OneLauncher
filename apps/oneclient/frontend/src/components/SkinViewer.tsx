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
}

export function SkinViewer({
	skinUrl,
	capeUrl,
	width = 260,
	height = 300,
	className,
}: SkinViewerProps) {
	const canvasRef = useRef<HTMLCanvasElement>(null);
	const viewerRef = useRef<skinviewer.SkinViewer | null>(null);

	useEffect(() => {
		if (!canvasRef.current)
			return;

		const viewer = new skinviewer.SkinViewer({
			canvas: canvasRef.current,
		});

		viewer.autoRotate = true;
		viewer.autoRotateSpeed = 0.25;

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

	return (
		<div className={twMerge('flex flex-col justify-center items-center', className)} style={{ minWidth: `${width}px`, minHeight: `${height}px` }}>
			<canvas
				height={height}
				ref={canvasRef}
				width={width}
			/>

			<span className="text-fg-secondary text-xs">Hold to drag. Scroll to zoom in/out.</span>
		</div>
	);
}
