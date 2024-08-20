import type { PlausibleOptions as PlausibleTrackerOptions } from 'plausible-tracker';
import Plausible from 'plausible-tracker';
import { useCallback, useEffect, useRef } from 'react';

import { createMutable } from 'solid-js/store';
import { createPersistedMutable, useSolidStore } from '../library';
import { PROGRAM_INFO, type ProgramInfo } from '../constants';
import { useDebugState } from './useDebug';

/**
 * Possible platform types for Plausible.
 */
export type PlausiblePlatformType = 'desktop' | 'unknown';

/**
 * The Plausible analytics state.
 */
export interface PlausibleState {
	shareFullTelemetry: boolean;
	platform: PlausiblePlatformType;
	programInfo: ProgramInfo;
}

/**
 * The mutable solid {@link PlausibleState} holder.
 */
export const plausibleState: PlausibleState = createPersistedMutable(
	'onelauncher-layout',
	createMutable<PlausibleState>({
		shareFullTelemetry: false, // false by default
		platform: 'unknown',
		programInfo: PROGRAM_INFO,
	}),
);

/**
 * A solid hook for using the solid store {@link plausibleState}.
 */
export const usePlausibleState = (): PlausibleState => useSolidStore(plausibleState);

const PlausibleProvider = Plausible({
	trackLocalhost: true,
	domain: 'app.polyfrost.org',
	apiHost: 'https://analytics.polyfrost.org',
});

/**
 * Defines all possible options that may be provided by events upon submission.
 *
 * Extends the {@link PlausibleTrackerOptions} provided by the `plausible-tracker`
 * package with additional options for custom functionality.
 */
export type PlausibleOptions = PlausibleTrackerOptions & { telemetryOverride?: boolean };

/**
 * A base Plausible event, that all other events are derived from for type safety.
 */
interface BasePlausibleEventWithOption<T, O extends keyof PlausibleOptions> {
	type: T;
	plausibleOptions: Required<{ [K in O]: PlausibleOptions[O]; }>;
}

interface BasePlausibleEventWithoutOption<T> { type: T }

type BasePlausibleEvent<T, O = void> = O extends keyof PlausibleOptions
	? BasePlausibleEventWithOption<T, O>
	: BasePlausibleEventWithoutOption<T>;

// Plausible events
type PageViewEvent = BasePlausibleEvent<'pageview', 'url'>;
type ClusterCreateEvent = BasePlausibleEvent<'clusterCreate'>;
type ClusterDeleteEvent = BasePlausibleEvent<'clusterDelete'>;
type ModAddedEvent = BasePlausibleEvent<'modAdded'>;
type ModRemovedEvent = BasePlausibleEvent<'modRemoved'>;
type PingEvent = BasePlausibleEvent<'ping'>;

/** All plausible events, added as goals for the currently active domain. */
type PlausibleEvent =
	| PageViewEvent
	| ClusterCreateEvent
	| ClusterDeleteEvent
	| ModAddedEvent
	| ModRemovedEvent
	| PingEvent;

/**
 * A wrapper for Plausible events to be handled internally.
 *
 * Allows for in-depth event logging to console and Plausible.
 */
interface PlausibleTrackerEvent {
	eventName: string;
	props: {
		platform: PlausiblePlatformType;
		fullTelemetry: boolean;
		coreVersion: string;
		commit: string;
		debug: boolean;
	};
	options: PlausibleTrackerOptions;
	readonly callback?: () => void;
}

interface SubmitEventProps {
	/** The {@link PlausibleEvent plausible event} to submit. */
	event: PlausibleEvent;
	/** The current {@link PlausiblePlatformType platform type}. */
	platformType: PlausiblePlatformType;
	/** An optional screen width. Defaults to `window.screen.width` */
	screenWidth?: number;
	/** Whether or not full telemetry is enabled for the current client. `usePlausibleState().shareFullTelemetry` */
	shareFullTelemetry: boolean;
	/** Whether or not debug is enabled for the current client. */
	debugState: {
		enabled: boolean;
		shareFullTelemetry: boolean;
		telemetryLogging: boolean;
	};
	programInfo: ProgramInfo;
}

/**
 * Submits an event directly to our Plausible server.
 *
 * **Avoid using this directly, only send telemetry when it is certain that it has been
 * allowed by the user. Prefer the {@link usePlausibleEvent `usePlausibleEvent`} hook.**
 *
 * @remarks
 * If any of the conditions are met, this will return and no data will be submitted:
 *
 * - The app is in debug or development mode.
 * - A telemetry override is present and is not true.
 * - No telemetry override is present, and telemetry sharing is not true.
 *
 * Telemetry sharing settings are never matched to `=== false` but rather to
 * `!== true`. This is so we can gaurentee that **nothing** will be sent
 * unless the user explicitly allows it.
 *
 * @see {@link https://plausible.io/docs/custom-event-goals Plausible custom events}
 * @see {@link https://plausible-tracker.netlify.app/#tracking-custom-events-and-goals Tracking custom Plausible events}
 */
async function submitPlausibleEvent({ event, debugState, ...props }: SubmitEventProps): Promise<void> {
	if (props.platformType === 'unknown')
		return;

	if (debugState.enabled && debugState.shareFullTelemetry !== true)
		return;

	if (
		'plausibleOptions' in event && 'telemetryOverride' in event.plausibleOptions
			? event.plausibleOptions.telemetryOverride !== true
			: props.shareFullTelemetry !== true && event.type !== 'ping'
	)
		return;

	const fullEvent: PlausibleTrackerEvent = {
		eventName: event.type,
		props: {
			platform: props.platformType,
			fullTelemetry: props.shareFullTelemetry,
			coreVersion: props.programInfo.launcher_version,
			commit: props.programInfo.tauri_version,
			debug: debugState.enabled,
		},
		options: {
			domain: 'app.polyfrost.org',
			deviceWidth: props.screenWidth ?? window.screen.width,
			referrer: '',
			...('plausibleOptions' in event ? event.plausibleOptions : undefined),
		},
		callback: debugState.telemetryLogging
			? () => {
					const { callback: _, ...event } = fullEvent;
					// eslint-disable-next-line no-console -- debug info
					console.log(event);
				}
			: undefined,
	};

	PlausibleProvider.trackEvent(
		fullEvent.eventName,
		{
			props: fullEvent.props,
			callback: fullEvent.callback,
		},
		fullEvent.options,
	);
}

interface EventSubmissionCallbackProps {
	/** The {@link PlausibleEvent Plausible event} to be submitted. */
	event: PlausibleEvent;
}

/**
 * Submits a Plausible Analytics event with a hook.
 *
 * The returned callback will only the fired once as to avoid flooding analytics.
 *
 * Certain events provide functionality to override telemetry sharing. This is because
 * it should only be used in contexts where telemetry sharing must be allowed by external means.
 *
 * @remarks
 * If any of the conditions are met, this will return and no data will be submitted:
 *
 * - The app is in debug or development mode.
 * - A telemetry override is present and is not true.
 * - No telemetry override is present, and telemetry sharing is not true.
 *
 * @returns a callback that, once executed, will submit the event.
 *
 * @example
 * ```ts
 * const submitPlausibleEvent = usePlausibleEvent();
 *
 * const createdCluster = commands.createCluster('myCluster').then(cluster => {
 * 	submitPlausibleEvent({
 * 		event: {
 * 			type: 'clusterCreate',
 * 		},
 * 	});
 * 	return cluster;
 * });
 * ```
 */
export function usePlausibleEvent(): (props: EventSubmissionCallbackProps) => Promise<void> {
	const debugState = useDebugState();
	const plausibleState = usePlausibleState();
	const previousEvent = useRef({} as BasePlausibleEvent<string>);

	return useCallback(async (props: EventSubmissionCallbackProps) => {
		if (previousEvent.current === props.event)
			return;
		else previousEvent.current = props.event;

		submitPlausibleEvent({
			debugState,
			shareFullTelemetry: plausibleState.shareFullTelemetry,
			platformType: plausibleState.platform,
			programInfo: plausibleState.programInfo,
			...props,
		});
	}, [debugState, plausibleState]);
}

export interface PlausibleMonitorProps {
	/** A sanitized string containing the current active path the client is viewing. */
	currentPath: string;
}

/**
 * Submits a Plausible Analytics `'pageview'` event with a hook.
 *
 * This should watch the Solid router's current path, and send an event if the path is changed.
 *
 * @remarks
 * If any of the conditions are met, this will return and no data will be submitted:
 *
 * - The app is in debug or development mode.
 * - A telemetry override is present and is not true.
 * - No telemetry override is present, and telemetry sharing is not true.
 */
export function usePlausiblePageview({ currentPath }: PlausibleMonitorProps): void {
	const plausibleEvent = usePlausibleEvent();

	useEffect(() => {
		plausibleEvent({
			event: {
				type: 'pageview',
				plausibleOptions: { url: currentPath },
			},
		});
	}, [currentPath, plausibleEvent]);
}

/**
 * Submits a Plausible Analytics `'ping'` event with a hook.
 *
 * This should watch the Solid router's current path, and send an event if the path is changed.
 *
 * @remarks
 * If any of the conditions are met, this will return and no data will be submitted:
 *
 * - The app is in debug or development mode.
 * - A telemetry override is present and is not true.
 * - No telemetry override is present, and telemetry sharing is not true.
 */
export function usePlausiblePing({ currentPath }: PlausibleMonitorProps) {
	const plausibleEvent = usePlausibleEvent();

	useEffect(() => {
		plausibleEvent({
			event: {
				type: 'ping',
			},
		});
	}, [currentPath, plausibleEvent]);
}

export interface PlausibleInitProps {
	platformType: PlausiblePlatformType;
	programInfo: ProgramInfo;
}

export function initializePlausible(props: PlausibleInitProps): void {
	plausibleState.platform = props.platformType;
	plausibleState.programInfo = props.programInfo;
}
