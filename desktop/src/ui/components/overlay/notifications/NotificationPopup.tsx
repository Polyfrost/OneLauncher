import { For, Match, Switch, onMount } from 'solid-js';
import { Settings01Icon, Trash01Icon } from '@untitled-theme/icons-solid';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import Popup from '../Popup';
import Button from '../../base/Button';
import * as manager from '../../../../bridge/notifications';
import NotificationComponent from './NotificationComponent';
import useNotifications from '~ui/hooks/useNotifications';

function NotificationPopup(props: Popup.PopupProps) {
	const [notifications] = useNotifications(updateSize);

	let inner!: HTMLDivElement;
	let parent!: HTMLDivElement;

	function updateSize() {
		if (inner && parent) {
			const rect = inner.getBoundingClientRect();
			parent.style.height = `${rect.height}px`;
		}
	}

	onMount(() => {
		document.addEventListener('keypress', (e) => {
			if (e.key === 'n') {
				manager.addNotification({
					title: 'Test Notification',
					message: 'This is a test notification',
					notification_type: manager.NotificationType.Download,
					...(Math.random() > 0.7 ? { progress: 0.39 } : {}),
				});
			}
		});
	});

	return (
		<Popup {...props}>
			<div class="bg-secondary rounded-xl border border-gray-10 w-96 p-2 shadow-md shadow-black/30">
				<div class="overflow-hidden transition-height" ref={parent}>
					<div class="flex flex-col justify-start items-stretch text-start gap-2" ref={inner}>
						<p class="text-2lg px-2 pt-1">Notifications</p>
						<Switch>
							<Match when={notifications().length > 0}>
								<OverlayScrollbarsComponent class="max-h-[min(500px,60vh)] overflow-auto">
									<div class="flex flex-col-reverse justify-center items-stretch ">
										<For each={notifications()}>
											{noti => (
												<div class="flex flex-col w-full">
													<NotificationComponent data={noti} overlay={false} />
													<span class="bg-gray-05 h-px w-full" />
												</div>
											)}
										</For>
									</div>
								</OverlayScrollbarsComponent>
							</Match>
							<Match when={notifications().length === 0}>
								<span class="px-2">You have no notifications</span>
								<span class="bg-gray-05 h-px" />
							</Match>
						</Switch>

						<div class="flex flex-row justify-between items-end">
							<Button onClick={() => manager.clearNotifications()} buttonStyle="ghost" iconLeft={<Trash01Icon />}>
								Clear Notifications
							</Button>

							<Button buttonStyle="icon" large>
								<Settings01Icon />
							</Button>
						</div>
					</div>
				</div>
			</div>
		</Popup>
	);
}

export default NotificationPopup;
