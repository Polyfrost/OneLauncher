import type { BeforeLeaveEventArgs } from '@solidjs/router';
import { useBeforeLeave } from '@solidjs/router';
import { createSignal } from 'solid-js';

interface PreventLeaveContext {
	continue: () => void;
	cancel: () => void;
	preventNavigation: () => void;
	triedNavigating: () => boolean;
};

function usePreventLeave(listener: (ctx: PreventLeaveContext) => void): PreventLeaveContext {
	const [event, setEvent] = createSignal<BeforeLeaveEventArgs | null>(null);

	const ctx: PreventLeaveContext = {
		continue: () => {
			if (ctx.triedNavigating() !== true) {
				console.warn('PreventLeaveContext.continue() called without an event');
				return;
			}

			event()?.retry(true);
			setEvent(null);
		},

		cancel: () => {
			setEvent(null);
		},

		preventNavigation: () => event()?.preventDefault(),
		triedNavigating: () => event() !== null,
	};

	useBeforeLeave((e) => {
		setEvent(e);

		listener(ctx);
	});

	return ctx;
}

export default usePreventLeave;
