globalThis.isDevelopment = import.meta.env.DEV;

globalThis.onHotReload = (func: () => void) => {
	if (import.meta.hot)
		import.meta.hot.dispose(func);
};
