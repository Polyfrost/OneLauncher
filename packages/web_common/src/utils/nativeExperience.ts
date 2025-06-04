export function registerNativeExperience() {
	// disable browser right-click context menu
	document.addEventListener('contextmenu', (e) => {
		e.preventDefault();
	});
}
