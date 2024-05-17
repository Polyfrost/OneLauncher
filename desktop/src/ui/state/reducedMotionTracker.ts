import { createEffect, createRoot, createSignal, onMount } from 'solid-js';

function createReducedMotionTracker() {
	const [reducedMotion, setReducedMotion] = createSignal(false);

	onMount(() => {
		const enabled = window.localStorage.getItem('reduced-motion') === 'true';
		setReducedMotion(enabled);
	});

	createEffect(() => {
		const enabled = reducedMotion();
		document.body.classList.toggle('reduce-motion', enabled);
		window.localStorage.setItem('reduced-motion', enabled.toString());
	});

	return { reducedMotion, setReducedMotion };
}

export default createRoot(createReducedMotionTracker);
