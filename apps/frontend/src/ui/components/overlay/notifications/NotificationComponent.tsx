import type { IngressPayload, IngressType } from '@onelauncher/client/bindings';
import { PausableTimer } from '@onelauncher/client';
import { InfoCircleIcon } from '@untitled-theme/icons-solid';
import { type ComponentProps, createEffect, createSignal, type JSX, Match, Show, Switch } from 'solid-js';

type NotificationComponentProps = IngressPayload & {
	overlay: boolean;
};

function IconFromNotificationType(type: IngressType['type']): (props: ComponentProps<'svg'>) => JSX.Element {
	switch (type) {
		case 'initialize':
		case 'download_java':
		case 'download_loader':
		case 'sync_cluster':
		case 'copy_cluster':
		case 'sync_config':
		case 'archival':
		case 'download_package':
		case 'download_pack':
		default:
			return InfoCircleIcon;
	}
}

function ColorFromNotificationType(type: IngressType['type']): string {
	switch (type) {
		case 'initialize':
		case 'download_java':
		case 'download_loader':
		case 'sync_cluster':
		case 'copy_cluster':
		case 'sync_config':
		case 'archival':
		case 'download_package':
		case 'download_pack':
		default:
			return 'text-fg-primary';
	}
}

const TOTAL_SECONDS = 7;
function NotificationOverlayComponent(props: NotificationComponentProps) {
	const [disappearing, setDisappearing] = createSignal<boolean>(true);
	const [timer, setTimer] = createSignal<PausableTimer | undefined>();
	const [visible, setVisible] = createSignal<boolean>(true);
	const [secondsLeft, setSecondsLeft] = createSignal<number>(TOTAL_SECONDS);

	createEffect(() => {
		setDisappearing(props.fraction === undefined);

		if (props.fraction !== null && props.fraction >= 1) {
			setVisible(false);
			return;
		}

		if (disappearing()) {
			setTimer(new PausableTimer(onInterval, 1000, true));
			return;
		}

		if (timer()) {
			timer()?.stop();
			setTimer(undefined);
		}
	});

	// onMount(() => {
	// 	setVisible(true);
	// });

	function onInterval() {
		if (secondsLeft() <= 0) {
			hide();
			return;
		}

		setSecondsLeft(secondsLeft() - 1);
	}

	function hide() {
		timer()?.stop();
		setVisible(false);
		setTimer(undefined);
	}

	function onEnter() {
		timer()?.pause();
	}

	function onLeave() {
		timer()?.resume();
	}

	return (
	// <Transition
	// 	enterClass="noti-animation-enter"
	// 	enterActiveClass="noti-animation-enter-active"
	// 	enterToClass="noti-animation-enter-to"
	// 	exitClass="noti-animation-leave"
	// 	exitActiveClass="noti-animation-leave-active"
	// 	exitToClass="noti-animation-leave-to"
	// >
		<Show when={visible()}>
			<div
				class="flex flex-col overflow-hidden rounded-lg bg-component-bg"
				onMouseEnter={() => onEnter()}
				onMouseLeave={() => onLeave()}
			>
				<div class="px-2">
					<NotificationPopupComponent {...props} />
				</div>

				<Show when={disappearing() === true && props.fraction === undefined}>
					<div class="h-1.5 w-full bg-brand-disabled">
						<div
							class="h-1.5 rounded-lg bg-brand transition-width"
							style={{
								width: `${(secondsLeft() / TOTAL_SECONDS) * 100}%`,
							}}
						/>
					</div>
				</Show>
			</div>
		</Show>
	// </Transition>
	);
}

function NotificationPopupComponent(props: NotificationComponentProps) {
	return (
		<div class="flex flex-col gap-y-1 p-2">
			<div class="grid grid-cols-[24px_1fr_auto] min-h-10 place-items-center gap-3">
				{IconFromNotificationType(props.event.type)({
					class: `w-6 h-6 ${ColorFromNotificationType(props.event.type)}`,
				})}

				<div class="w-full flex flex-col">
					<span class={`font-medium ${ColorFromNotificationType(props.event.type)}`}>{props.message}</span>
					<span class="text-sm text-white/60">{props.message}</span>
				</div>

				<Show when={props.overlay !== true}>
					<div class="flex flex-row items-center justify-end gap-1">
						<span class="h-1.5 w-1.5 rounded-full bg-brand" />
					</div>
				</Show>
			</div>

			<Show when={props.fraction !== null}>
				<div class="h-1.5 w-full overflow-hidden rounded-full bg-brand-disabled">
					<div
						class="h-full max-w-full min-w-0 rounded-full bg-brand transition-width"
						style={{
							width: `${Math.floor(props.fraction!)}%`,
						}}
					/>
				</div>
			</Show>
		</div>
	);
}

function NotificationComponent(props: NotificationComponentProps) {
	return (
		<Switch>
			<Match when={props.overlay === true}>
				<NotificationOverlayComponent {...props} />
			</Match>
			<Match when={props.overlay !== true}>
				<NotificationPopupComponent {...props} />
			</Match>
		</Switch>
	);
}

export default NotificationComponent;
