import { useEffect, useState } from 'react';
import { createMutable } from 'solid-js/store';
import { createPersistedMutable, useSolidStore } from '../library';

export interface DebugState {
	enabled: boolean;
	shareFullTelemetry: boolean;
	telemetryLogging: boolean;
}

export const debugState = createPersistedMutable(
	'onelauncher-debugState',
	createMutable<DebugState>({
		enabled: globalThis.isDevelopment,
		shareFullTelemetry: false,
		telemetryLogging: false,
	}),
);

export const useDebugState = () => useSolidStore(debugState);

export function useDebugToggle(): () => void {
	const [toggled, setToggled] = useState(0);

	useEffect(() => {
		if (toggled >= 5)
			debugState.enabled = true;

		const timeout = setTimeout(() => setToggled(0), 1000);
		return () => clearTimeout(timeout);
	}, [toggled]);

	return () => setToggled(c => c + 1);
}
