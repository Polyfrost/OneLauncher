import { PausableTimer } from '@onelauncher/client';
import { InfoCircleIcon } from '@untitled-theme/icons-solid';
import { createEffect, createSignal, type JSX, Match, Show, Switch } from 'solid-js';

export interface NotificationData {
	title: string;
	message: string;
	fraction?: number | undefined;
}

export type NotificationComponentProps = NotificationData & {
	icon?: () => JSX.Element;
	overlay: boolean;
};

const TOTAL_SECONDS = 7;
const fractionEnded = (fraction: number | undefined) => fraction !== undefined && fraction >= 0.97;

function NotificationOverlayComponent(props: NotificationComponentProps) {
	const [disappearing, setDisappearing] = createSignal<boolean>(true);
	const [timer, setTimer] = createSignal<PausableTimer | undefined>();
	const [visible, setVisible] = createSignal<boolean>(true);
	const [secondsLeft, setSecondsLeft] = createSignal<number>(TOTAL_SECONDS);

	createEffect(() => {
		setDisappearing(props.fraction === undefined);

		if (fractionEnded(props.fraction)) {
			setVisible(false);
			return;
		}

		if (disappearing()) {
			setTimer(new PausableTimer(onInterval, 1000, true));
			return;
		}

		if (timer()) {
			timer()?.stop();
			setTimer(undefined);
		}
	});

	function onInterval() {
		if (secondsLeft() <= 0) {
			hide();
			return;
		}

		setSecondsLeft(secondsLeft() - 1);
	}

	function hide() {
		timer()?.stop();
		setVisible(false);
		setTimer(undefined);
	}

	function onEnter() {
		timer()?.pause();
	}

	function onLeave() {
		timer()?.resume();
	}

	return (
		<Show when={visible()}>
			<div
				class="flex flex-col overflow-hidden rounded-lg bg-component-bg"
				onMouseEnter={() => onEnter()}
				onMouseLeave={() => onLeave()}
			>
				<div class="px-2">
					<NotificationPopupComponent {...props} />
				</div>

				<Show when={disappearing() === true}>
					<div class="h-1.5 w-full bg-brand-disabled">
						<div
							class="h-1.5 rounded-lg bg-brand transition-width"
							style={{
								width: `${(secondsLeft() / TOTAL_SECONDS) * 100}%`,
							}}
						/>
					</div>
				</Show>
			</div>
		</Show>
	);
}

function NotificationPopupComponent(props: NotificationComponentProps) {
	// eslint-disable-next-line solid/reactivity -- Doesn't really matter here
	const Icon = props.icon ?? InfoCircleIcon;

	return (
		<div class="flex flex-col gap-y-1 p-2">
			<div class="grid grid-cols-[24px_1fr_auto] min-h-10 place-items-center gap-3">
				<Icon class="h-6 w-6 text-fg-primary" />

				<div class="w-full flex flex-col gap-y-1">
					<span class="text-fg-primary font-medium capitalize">{props.title}</span>
					<span class="text-sm text-white/60 capitalize">{props.message}</span>
				</div>

				<Show when={props.overlay !== true}>
					<div class="flex flex-row items-center justify-end gap-1">
						<span class="h-1.5 w-1.5 rounded-full bg-brand" />
					</div>
				</Show>
			</div>

			<Show when={props.fraction !== undefined && props.fraction !== null && !fractionEnded(props.fraction)}>
				<div class="h-1.5 w-full overflow-hidden rounded-full bg-brand-disabled">
					<div
						class="h-full max-w-full min-w-0 rounded-full bg-brand transition-width"
						style={{
							width: `${Math.floor(props.fraction! * 100)}%`,
						}}
					/>
				</div>
			</Show>
		</div>
	);
}

function NotificationComponent(props: NotificationComponentProps) {
	return (
		<Switch>
			<Match when={props.overlay === true}>
				<NotificationOverlayComponent {...props} />
			</Match>
			<Match when={props.overlay !== true}>
				<NotificationPopupComponent {...props} />
			</Match>
		</Switch>
	);
}

export default NotificationComponent;
