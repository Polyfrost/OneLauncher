(() => {
	if (location.hostname !== 'localhost')
		return;

	window.addEventListener('keypress', (e) => {
		if (e.getModifierState('Control') && e.key === 'r') {
			e.preventDefault();
			window.location.reload(); // tauri disables this i think
		}
	});
})();
