import { For } from 'solid-js';
import Popup from '../Popup';
import NotificationComponent from './NotificationComponent';
import useNotifications from '~ui/hooks/useNotifications';

export default function NotificationOverlay() {
	const [notifications] = useNotifications();

	return (
		<Popup
			setVisible={() => {}}
			visible={() => true}
			mount={document.body}
			ref={ref => ref.classList.add('fixed', 'bottom-8', 'right-8')}
		>
			<div class="flex flex-col-reverse gap-2">
				<For each={notifications()}>
					{notification => (
						<NotificationComponent overlay {...notification} />
					)}
				</For>
			</div>
		</Popup>
	);
}
