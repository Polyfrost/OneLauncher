import { For } from 'solid-js';
import Popup from '../Popup';
import NotificationComponent from './NotificationComponent';
import useIngress from '~ui/hooks/useIngress';

export default function NotificationOverlay() {
	const [ingress] = useIngress();

	return (
		<Popup
			setVisible={() => {}}
			visible={() => true}
			mount={document.body}
			ref={ref => ref.classList.add('fixed', 'bottom-8', 'right-8')}
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
