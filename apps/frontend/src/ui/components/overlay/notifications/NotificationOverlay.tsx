import useIngress from '~ui/hooks/useIngress';
import { For } from 'solid-js';
import Popup from '../Popup';
import NotificationComponent from './NotificationComponent';

export default function NotificationOverlay() {
	const [ingress] = useIngress();

	return (
		<Popup
			mount={document.body}
			ref={ref => ref.classList.add('fixed', 'bottom-8', 'right-8')}
			setVisible={() => {}}
			visible={() => true}
		>
			<div class="flex flex-col-reverse gap-2">
				<For each={ingress()}>
					{notification => (
						<NotificationComponent overlay {...notification} />
					)}
				</For>
			</div>
		</Popup>
	);
}
