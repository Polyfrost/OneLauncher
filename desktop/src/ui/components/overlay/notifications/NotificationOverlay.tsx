import { For, createSignal } from 'solid-js';
import Popup from '../Popup';
import NotificationComponent from './NotificationComponent';
import useNotifications from '~ui/hooks/useNotifications';
import createPausableTimer from '~utils/PausableTimer';

export default function NotificationOverlay() {
	const [notifications] = useNotifications();

	// function onAdded() {
	// 	const noti = notifications()[notifications().length - 1];
	// 	if (noti) {
	// 		displayed().push(noti);
	// 		setDisplayed([...displayed()]);
	// 		const id = noti.id;

	// 		const pausable = createPausableTimer(() => {
	// 			if (!notifications().includes(noti))
	// 				return;

	// 			const index = displayed().findIndex(noti => noti.id === id);

	// 			if (index > -1) {
	// 				displayed().splice(index, 1);
	// 				setDisplayed([...displayed()]);
	// 			}
	// 		}, 2500);
	// 	}
	// }

	// function onRemoved() {
	// 	const index = displayed().findIndex(noti => !notifications().includes(noti));
	// 	if (index > -1) {
	// 		displayed().splice(index, 1);
	// 		setDisplayed([...displayed()]);
	// 	}
	// }

	// function onClear() {
	// 	setDisplayed([]);
	// }

	return (
		<Popup
			setVisible={() => {}}
			visible={() => true}
			mount={document.body}
			ref={ref => ref.classList.add('fixed', 'bottom-8', 'right-8')}
		>
			<div class="flex flex-col gap-2">
				<For each={notifications()}>
					{notification => (
						<NotificationComponent overlay data={notification} />
					)}
				</For>
			</div>
		</Popup>
	);
}
