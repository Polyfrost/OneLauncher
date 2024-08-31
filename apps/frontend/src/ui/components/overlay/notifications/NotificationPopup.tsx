import { For, Match, Switch } from 'solid-js';
import { Settings01Icon, Trash01Icon } from '@untitled-theme/icons-solid';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import Popup, { type PopupProps } from '../Popup';
import Button from '../../base/Button';
import NotificationComponent from './NotificationComponent';
import useIngress from '~ui/hooks/useIngress';

function NotificationPopup(props: PopupProps) {
	const [ingress] = useIngress(updateSize);

	let inner!: HTMLDivElement;
	let parent!: HTMLDivElement;

	function updateSize() {
		if (inner && parent) {
			const rect = inner.getBoundingClientRect();
			parent.style.height = `${rect.height}px`;
		}
	}

	// onMount(() => {
	// 	document.addEventListener('keypress', async (e) => {
	// 		if (e.key === 'n') {
	// 			const progress = Math.random() > 0.0 ? { progress: 0.39 } : {};
	// 			const id = await manager.addNotification({
	// 				title: 'Test Notification',
	// 				message: 'This is a test notification',
	// 				notification_type: manager.NotificationType.Download,
	// 				...(progress),
	// 			});
	// 		}
	// 	});
	// });

	return (
		<Popup {...props}>
			<div class="w-96 border border-gray-10 rounded-xl bg-page-elevated p-2 shadow-black/30 shadow-md">
				<div class="overflow-hidden transition-height" ref={parent}>
					<div class="flex flex-col items-stretch justify-start gap-2 text-start" ref={inner}>
						<p class="px-2 pt-1 text-2lg">Notifications</p>
						<Switch>
							<Match when={ingress().length > 0}>
								<OverlayScrollbarsComponent class="max-h-[min(500px,60vh)] overflow-auto">
									<div class="flex flex-col-reverse items-stretch justify-center">
										<For each={ingress()}>
											{noti => (
												<div class="w-full flex flex-col">
													<NotificationComponent {...noti} overlay={false} />
													<span class="h-px w-full bg-gray-05" />
												</div>
											)}
										</For>
									</div>
								</OverlayScrollbarsComponent>
							</Match>
							<Match when={ingress().length === 0}>
								<span class="px-2">You have no notifications</span>
								<span class="h-px bg-gray-05" />
							</Match>
						</Switch>

						<div class="flex flex-row items-end justify-between">
							<Button onClick={() => {}} buttonStyle="ghost" iconLeft={<Trash01Icon />}>
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
