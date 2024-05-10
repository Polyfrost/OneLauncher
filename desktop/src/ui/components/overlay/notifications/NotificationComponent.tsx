import { type JSX, Show } from 'solid-js';
import { AlertTriangleIcon, CheckCircleIcon, FolderCheckIcon, FolderDownloadIcon, FolderXIcon, InfoCircleIcon, RefreshCcw02Icon } from '@untitled-theme/icons-solid';
import TimeAgo from '../../TimeAgo';
import { NotificationType } from '../../../../bridge/notifications';

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

function NotificationOverlayComponent(props: NotificationComponentProps) {
	return (
		<div>
			<span>{props.data.title}</span>
		</div>
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

				<div class="flex flex-row justify-end items-center gap-1">
					<span class="text-sm text-white/40">
						<TimeAgo timestamp={props.data.created_at * 1000} />
					</span>
					<span class="w-1.5 h-1.5 rounded-full bg-brand" />
				</div>
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
