import type { NotificationData } from '@/hooks/useNotification';
import useNotifications from '@/hooks/useNotification';
import { Button, Menu, Popup } from '@onelauncher/common/components';
import { XIcon } from '@untitled-theme/icons-react';
import { Popover } from 'react-aria-components';

interface NotificationItemProps {
	id: string;
	data: NotificationData;
	onRemove: (id: string) => void;
}

function NotificationItem({ id, data, onRemove }: NotificationItemProps) {
	return (
		<div className="flex flex-col gap-2 p-3 bg-component-bg border border-component-border rounded-lg">
			<div className="flex items-start justify-between gap-2">
				<div className="flex-1 min-w-0">
					<h4 className="font-semibold text-fg-primary text-sm truncate">
						{data.title}
					</h4>
					<p className="text-fg-secondary text-xs mt-1 break-words">
						{data.message}
					</p>
				</div>
				<Button
					className="flex-shrink-0 h-6 w-6 p-0"
					color="ghost"
					onClick={() => onRemove(id)}
					size="icon"
				>
					<XIcon className="h-3 w-3" />
				</Button>
			</div>

			{data.fraction !== undefined && (
				<div className="w-full bg-component-bg-hover rounded-full h-2">
					<div
						className="bg-brand h-2 rounded-full transition-all duration-300"
						style={{ width: `${Math.max(0, Math.min(100, data.fraction))}%` }}
					/>
				</div>
			)}
		</div>
	);
}

function NotificationPopup() {
	const { list, clear, remove } = useNotifications();
	const notificationEntries = Object.entries(list);

	const removeNotification = (id: string) => {
		remove(id);
	};

	if (notificationEntries.length === 0)
		return (
			<div className="p-3 w-80 text-center">
				<p className="text-fg-secondary text-sm">No notifications</p>
			</div>
		);

	return (
		<Popup className="min-w-80 w-80 max-w-96 max-h-96 overflow-y-auto">
			<div className="flex items-center justify-between border-b border-component-border">
				<h3 className="font-semibold text-fg-primary">Notifications</h3>
				{notificationEntries.length > 0 && (
					<Button
						className="text-xs"
						color="ghost"
						onClick={clear}
						size="normal"
					>
						Clear All
					</Button>
				)}
			</div>

			{notificationEntries.map(([id, data]) => (
				<div key={id}>
					<div className="p-2">
						<NotificationItem
							data={data}
							id={id}
							onRemove={removeNotification}
						/>
					</div>
				</div>
			))}
		</Popup>
	);
}

export default NotificationPopup;
