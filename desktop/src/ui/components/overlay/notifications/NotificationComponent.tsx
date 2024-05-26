import { type JSX, Show, createEffect, createSignal } from 'solid-js';
import { AlertTriangleIcon, CheckCircleIcon, FolderCheckIcon, FolderDownloadIcon, FolderXIcon, InfoCircleIcon, RefreshCcw02Icon } from '@untitled-theme/icons-solid';
import TimeAgo from '../../TimeAgo';
import { NotificationType } from '../../../../bridge/notifications';
import { PausableTimer } from '~utils/PausableTimer';

interface NotificationComponentProps {
	overlay: boolean;
	data: Core.Notification;
};

function IconFromNotificationType(type: NotificationType): (props: JSX.HTMLAttributes<HTMLDivElement>) => JSX.Element {
	switch (type) {
		case NotificationType.Alert:
			return AlertTriangleIcon as any;
		case NotificationType.Download:
			return FolderDownloadIcon as any;
		case NotificationType.DownloadSuccess:
			return FolderCheckIcon as any;
		case NotificationType.DownloadError:
			return FolderXIcon as any;
		case NotificationType.Refresh:
			return RefreshCcw02Icon as any;
		case NotificationType.Success:
			return CheckCircleIcon as any;
		case NotificationType.Info:
		default:
			return InfoCircleIcon as any;
	}
}

function ColorFromNotificationType(type: NotificationType): string {
	switch (type) {
		case NotificationType.Info:
		case NotificationType.Refresh:
			return 'text-brand';

		case NotificationType.Alert:
		case NotificationType.DownloadError:
			return 'text-danger';

		case NotificationType.DownloadSuccess:
		case NotificationType.Success:
			return 'text-success';

		case NotificationType.Download:
		default:
			return 'text-fg-primary';
	}
}

const TOTAL_SECONDS = 7.5;
// TODO(refactor): Use only an interval for this (Remove the pausable signal and in the interval, check if elapsed is longer than duraihon)
function NotificationOverlayComponent(props: NotificationComponentProps) {
	const [disappearing, setDisappearing] = createSignal<boolean>(true);
	const [pausable, setPausable] = createSignal<PausableTimer | undefined>();
	const [interval, setSecondsInterval] = createSignal<PausableTimer | undefined>();
	const [visible, setVisible] = createSignal<boolean>(true);
	const [secondsLeft, setSecondsLeft] = createSignal<number>(TOTAL_SECONDS);

	createEffect(() => {
		setDisappearing(props.data.progress === undefined || props.data.progress > 1);

		if (disappearing()) {
			initTimers();
		}
		else if (pausable()) {
			clearTimeout(pausable()?.timeout);
			setPausable(undefined);
		}
	});

	function initTimers() {
		setPausable(new PausableTimer(hide, (TOTAL_SECONDS + 0.5) * 1000));
		setSecondsInterval(new PausableTimer(onInterval, 1000, true));
	}

	function onInterval() {
		setSecondsLeft(secondsLeft() - 1);
	}

	function hide() {
		setVisible(false);
		interval()?.stop();
		setSecondsInterval(undefined);
	}

	function onEnter() {
		pausable()?.pause();
		interval()?.pause();
	}

	function onLeave() {
		pausable()?.resume();
		interval()?.resume();
	}

	return (
		<Show when={visible()}>
			<div
				onMouseEnter={() => onEnter()}
				onMouseLeave={() => onLeave()}
				class="flex flex-col overflow-hidden rounded-lg bg-component-bg"
			>
				<div class="px-2">
					<NotificationPopupComponent {...props} />
				</div>

				<Show when={disappearing()}>
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
	);
}

function NotificationPopupComponent(props: NotificationComponentProps) {
	return (
		<div class="p-2 flex flex-col gap-y-1">
			<div class="min-h-10 grid place-items-center grid-cols-[24px_1fr_auto] gap-3">
				{IconFromNotificationType(props.data.notification_type)({
					class: `w-6 h-6 ${ColorFromNotificationType(props.data.notification_type)}`,
				})}

				<div class="flex flex-col w-full">
					<span class={`font-medium ${ColorFromNotificationType(props.data.notification_type)}`}>{props.data.title}</span>
					<span class="text-sm text-white/60">{props.data.message}</span>
				</div>

				<Show when={props.overlay !== true}>
					<div class="flex flex-row justify-end items-center gap-1">
						<span class="text-sm text-white/40">
							<TimeAgo timestamp={props.data.created_at * 1000} />
						</span>
						<span class="w-1.5 h-1.5 rounded-full bg-brand" />
					</div>
				</Show>
			</div>

			<Show when={props.data.progress}>
				<div class="rounded-full overflow-hidden h-1.5 bg-brand-disabled w-full">
					<div
						class="rounded-full h-full min-w-0 max-w-full bg-brand transition-[width]"
						style={{
							width: `${Math.floor(props.data.progress! * 100)}%`,
						}}
					/>
				</div>
			</Show>
		</div>
	);
}

function NotificationComponent(props: NotificationComponentProps) {
	return (
		<>
			{(props.overlay ? NotificationOverlayComponent : NotificationPopupComponent)(props)}
		</>
	);
}

export default NotificationComponent;
