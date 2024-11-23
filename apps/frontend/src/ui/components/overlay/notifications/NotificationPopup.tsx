import { Settings01Icon, Trash01Icon } from '@untitled-theme/icons-solid';
import useNotifications from '~ui/hooks/useNotifications';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { createEffect, createMemo, For, Match, on, Switch } from 'solid-js';
import Button from '../../base/Button';
import Popup, { type PopupProps } from '../Popup';
import NotificationComponent from './NotificationComponent';

function NotificationPopup(props: PopupProps) {
	const [notifications, setNotifications] = useNotifications();

	let inner!: HTMLDivElement;
	let parent!: HTMLDivElement;

	createEffect(on(notifications, () => {
		if (inner && parent) {
			const rect = inner.getBoundingClientRect();
			parent.style.height = `${rect.height}px`;
		}
	}));

	const memoedNotifications = createMemo(() => Object.values(notifications()));

	function clearNotifications() {
		setNotifications({});
	}

	return (
		<Popup {...props}>
			<div class="w-96 border border-border/10 rounded-xl bg-page-elevated p-2 shadow-black/30 shadow-md">
				<div class="overflow-hidden transition-height" ref={parent}>
					<div class="flex flex-col items-stretch justify-start gap-2 text-start" ref={inner}>
						<p class="px-2 pt-1 text-2lg">Notifications</p>
						<Switch>
							<Match when={memoedNotifications().length > 0}>
								<OverlayScrollbarsComponent class="max-h-[min(500px,60vh)] overflow-auto">
									<div class="flex flex-col-reverse items-stretch justify-center">
										<For each={memoedNotifications()}>
											{noti => (
												<div class="w-full flex flex-col">
													<NotificationComponent {...noti} overlay={false} />
													<span class="h-px w-full bg-border/05" />
												</div>
											)}
										</For>
									</div>
								</OverlayScrollbarsComponent>
							</Match>
							<Match when>
								<span class="px-2">You have no notifications</span>
								<span class="h-px bg-border/05" />
							</Match>
						</Switch>

						<div class="flex flex-row items-end justify-between">
							<Button buttonStyle="ghost" iconLeft={<Trash01Icon />} onClick={clearNotifications}>
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
