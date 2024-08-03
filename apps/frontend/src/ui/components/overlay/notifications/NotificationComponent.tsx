import { type JSX, Match, Show, Switch, createEffect, createSignal } from 'solid-js';
import { InfoCircleIcon } from '@untitled-theme/icons-solid';
import TimeAgo from '../../TimeAgo';
import { PausableTimer } from '~utils';

type NotificationComponentProps = Core.Notification & {
	overlay: boolean;
};

type NotificationType = '';

function IconFromNotificationType(type: NotificationType): (props: JSX.HTMLAttributes<HTMLDivElement>) => JSX.Element {
	switch (type) {
		// case NotificationType.Alert:
		// 	return AlertTriangleIcon as any;
		// case NotificationType.Download:
		// 	return FolderDownloadIcon as any;
		// case NotificationType.DownloadSuccess:
		// 	return FolderCheckIcon as any;
		// case NotificationType.DownloadError:
		// 	return FolderXIcon as any;
		// case NotificationType.Refresh:
		// 	return RefreshCcw02Icon as any;
		// case NotificationType.Success:
		// 	return CheckCircleIcon as any;
		// case NotificationType.Info:
		default:
			return InfoCircleIcon as any;
	}
}

function ColorFromNotificationType(type: NotificationType): string {
	switch (type) {
		// case NotificationType.Info:
		// case NotificationType.Refresh:
		// 	return 'text-brand';

		// case NotificationType.Alert:
		// case NotificationType.DownloadError:
		// 	return 'text-danger';

		// case NotificationType.DownloadSuccess:
		// case NotificationType.Success:
		// 	return 'text-success';

		// case NotificationType.Download:
		default:
			return 'text-fg-primary';
	}
}

const TOTAL_SECONDS = 7;
function NotificationOverlayComponent(props: NotificationComponentProps) {
	const [disappearing, setDisappearing] = createSignal<boolean>(true);
	const [timer, setTimer] = createSignal<PausableTimer | undefined>();
	const [visible, setVisible] = createSignal<boolean>(true);
	const [secondsLeft, setSecondsLeft] = createSignal<number>(TOTAL_SECONDS);

	createEffect(() => {
		setDisappearing(props.progress === undefined);

		if (props.progress !== undefined && props.progress >= 1) {
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

	// onMount(() => {
	// 	setVisible(true);
	// });

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
	// <Transition
	// 	enterClass="noti-animation-enter"
	// 	enterActiveClass="noti-animation-enter-active"
	// 	enterToClass="noti-animation-enter-to"
	// 	exitClass="noti-animation-leave"
	// 	exitActiveClass="noti-animation-leave-active"
	// 	exitToClass="noti-animation-leave-to"
	// >
		<Show when={visible()}>
			<div
				onMouseEnter={() => onEnter()}
				onMouseLeave={() => onLeave()}
				class="flex flex-col overflow-hidden rounded-lg bg-component-bg"
			>
				<div class="px-2">
					<NotificationPopupComponent {...props} />
				</div>

				<Show when={disappearing() === true && props.progress === undefined}>
					<div class="w-full h-1.5 bg-brand-disabled">
						<div
							style={{
								width: `${(secondsLeft() / TOTAL_SECONDS) * 100}%`,
							}}
							class="transition-width h-1.5 bg-brand rounded-lg"
						/>
					</div>
				</Show>
			</div>
		</Show>
	// </Transition>
	);
}

function NotificationPopupComponent(props: NotificationComponentProps) {
	return (
		<div class="p-2 flex flex-col gap-y-1">
			<div class="min-h-10 grid place-items-center grid-cols-[24px_1fr_auto] gap-3">
				{IconFromNotificationType(props.notification_type)({
					class: `w-6 h-6 ${ColorFromNotificationType(props.notification_type)}`,
				})}

				<div class="flex flex-col w-full">
					<span class={`font-medium ${ColorFromNotificationType(props.notification_type)}`}>{props.title}</span>
					<span class="text-sm text-white/60">{props.message}</span>
				</div>

				<Show when={props.overlay !== true}>
					<div class="flex flex-row justify-end items-center gap-1">
						<span class="text-sm text-white/40">
							<TimeAgo timestamp={props.created_at * 1000} />
						</span>
						<span class="w-1.5 h-1.5 rounded-full bg-brand" />
					</div>
				</Show>
			</div>

			<Show when={props.progress !== undefined}>
				<div class="rounded-full overflow-hidden h-1.5 bg-brand-disabled w-full">
					<div
						class="rounded-full h-full min-w-0 max-w-full bg-brand transition-width"
						style={{
							width: `${Math.floor(props.progress! * 100)}%`,
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
