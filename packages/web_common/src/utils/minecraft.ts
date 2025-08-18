export async function renderHeadToDataUrl(image: HTMLImageElement | string): Promise<string> {
	return new Promise((resolve) => {
		let img: CanvasImageSource;

		if (typeof image === 'string') {
			img = new Image();
			img.crossOrigin = 'anonymous';
			img.src = image;
		}
		else {
			img = image;
		}

		img.onload = () => {
			const canvas = document.createElement('canvas');
			const ctx = canvas.getContext('2d')!;
			canvas.width = 8;
			canvas.height = 8;

			// Base head (8x8 from (8,8))
			ctx.drawImage(img, 8, 8, 8, 8, 0, 0, 8, 8);

			// Overlay (8x8 from (40,8))
			ctx.drawImage(img, 40, 8, 8, 8, 0, 0, 8, 8);

			resolve(canvas.toDataURL('image/png'));
		};
	});
}
