import useNotifications from '@/hooks/useNotification';
import { Button } from '@onelauncher/common/components';
import { InfoCircleIcon, Trash01Icon } from '@untitled-theme/icons-react';

function NotificationPopup() {
	const { list, clear } = useNotifications();
	const notificationEntries = Object.entries(list);

	if (notificationEntries.length === 0)
		return (
			<div className="p-3 w-80 text-center">
				<p className="text-fg-secondary text-sm">No notifications</p>
			</div>
		);

	return (
		<div className="min-w-80 w-80 max-w-96 max-h-96 overflow-y-auto">
			<div className="flex flex-col bg-page-elevated">
				<div className="flex items-center justify-between mb-3">
					<h4 className="text-fg-primary ml-2">Notifications</h4>
				</div>

				<div className="flex flex-col gap-2 mb-3">
					{notificationEntries.map(([id, data]) => (
						<div className="flex items-start gap-3 p-2 rounded-lg" key={id}>
							<div className="flex-shrink-0 self-center">
								<InfoCircleIcon className="h-6 w-6 text-fg-primary" />
							</div>

							<div className="flex-1 min-w-0">
								<h4 className="font-semibold text-fg-primary text-sm">
									{data.title}
								</h4>
								<p className="text-fg-secondary text-xs mt-1">
									{data.message}
								</p>
							</div>

							<div className="flex-shrink-0 mt-2 self-center">
								<div className="h-2 w-2 rounded-full bg-blue-500" />
							</div>
						</div>
					))}
				</div>

				<hr className="mb-2 text-component-border-pressed" />

				{notificationEntries.length > 0 && (
					<div className="flex items-center justify-between">
						<Button
							className="text-left"
							color="ghost"
							onClick={clear}
							size="normal"
						>
							<Trash01Icon className="h-4 w-4" />
							Clear Notifications
						</Button>
						{/* <Button
							className="h-6 w-6 p-0"
							color="ghost"
							size="iconLarge"
						>
							<Settings01Icon />
						</Button> */}
					</div>
				)}
			</div>
		</div>
	);
}

export default NotificationPopup;
