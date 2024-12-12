import useNotifications from '~ui/hooks/useNotifications';
import { For } from 'solid-js';
import Popup from '../Popup';
import NotificationComponent from './NotificationComponent';

export default function NotificationOverlay() {
	const [notifications] = useNotifications();

	return (
		<Popup
			mount={document.body}
			ref={ref => ref.classList.add('fixed', 'bottom-8', 'right-8', 'z-99999')}
			setVisible={() => {}}
			visible={() => true}
		>
			<div class="flex flex-col-reverse gap-2">
				<For each={Object.values(notifications())}>
					{notification => (
						<NotificationComponent overlay {...notification} />
					)}
				</For>
			</div>
		</Popup>
	);
}
