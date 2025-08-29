import type { ToastData, ToastId } from '@/utils/toast';
import type { HTMLAttributes } from 'react';
import type { ToastContentProps, ToastOptions } from 'react-toastify';
import { bindings } from '@/main';
import { toast, toastUpdate } from '@/utils/toast';
import { Button } from '@onelauncher/common/components';
import { AlertCircleIcon, AlertTriangleIcon, CheckCircleIcon, InfoCircleIcon, XCloseIcon } from '@untitled-theme/icons-react';
import { useEffect } from 'react';
import { cssTransition, ToastContainer } from 'react-toastify';
import { tv } from 'tailwind-variants';
import { Spinner } from '../Spinner';
import styles from './Toasts.module.css';

const toastVariants = tv({
	slots: {
		container: [
			'relative h-full flex flex-row w-full justify-start items-center gap-3 py-4 px-4 rounded-lg overflow-hidden',
			'inset-ring inset-ring-component-border bg-component-bg',
		],
		closeBtn: [
			'absolute top-2 right-2',
		],
	},
	variants: {
		clickable: {
			true: {
				container: [
					'hover:bg-component-bg-hover active:bg-component-bg-pressed',
				],
			},
			false: {
				container: [],
			},
		},
	},
});

export function Toast({
	closeToast,
	data,
	toastProps,
	isPaused,
}: ToastContentProps<ToastData>) {
	const { container, closeBtn } = toastVariants({
		clickable: toastProps.closeOnClick || typeof toastProps.onClick !== 'undefined',
	});

	return (
		<div className={container()}>
			<ToastIcon isLoading={toastProps.isLoading} type={toastProps.type} />

			<div className="flex flex-col">
				<h4 className="text-lg font-medium">{data.title}</h4>
				{data.message && <p className="text-sm text-fg-secondary">{data.message}</p>}
			</div>

			<div className={closeBtn()}>
				<Button
					color="ghost"
					onClick={closeToast}
					size="icon"
				>
					<XCloseIcon />
				</Button>
			</div>

			<div className="w-full h-1 absolute left-0 right-0 bottom-0">
				<ToastProgressBar closeToast={closeToast} isPaused={isPaused} toastProps={toastProps} />
			</div>
		</div>
	);
}

function ToastProgressBar({
	isPaused,
	toastProps,
	closeToast,
}: {
	toastProps: ToastOptions;
	isPaused: boolean;
	closeToast: () => void;
}) {
	const attrs: HTMLAttributes<HTMLDivElement> = {};

	if (typeof toastProps.progress === 'number') {
		attrs.style = {
			transition: 'width 0.2s ease-in-out',
			width: `${toastProps.progress * 100}%`,
		};

		if (toastProps.progress >= 1)
			attrs.onTransitionEnd = closeToast;
	}
	else {
		attrs.style = {
			animationName: styles['custom-anim-progress-bar'],
			animationDuration: `${toastProps.autoClose}ms`,
			animationPlayState: isPaused ? 'paused' : 'running',
			animationTimingFunction: 'linear',
			animationDelay: '500ms',
		};

		attrs.onAnimationEnd = closeToast;
	}

	return (
		<div
			className="h-1 bg-brand"
			{...attrs}
		>
		</div>
	);
}

export function Toasts() {
	// runs twice in dev mode because of react strict mode
	// the double notifications are NOT a bug!!!
	useEffect(() => {
		let unlisten: (() => void) | undefined;
		const toastIngressMap: Map<string, ToastId> = new Map();

		bindings.events.ingress.on((e) => {
			const title = typeof e.ingress_type === 'string' ? e.ingress_type : Object.keys(e.ingress_type).at(0) ?? 'Info';

			const existingToastId = toastIngressMap.get(e.id);

			if (existingToastId) {
				toastUpdate(existingToastId, {
					progress: e.percent ?? 1,
					isLoading: true,
					message: e.message,
					title,
				});
			}
			else {
				const toastId = toast({
					type: 'info',
					title,
					progress: e.percent ?? 1,
					isLoading: true,
					message: e.message,
				});

				toastIngressMap.set(e.id, toastId);
			}
		}).then((fn) => {
			unlisten = fn;
		});

		return () => {
			if (unlisten)
				unlisten();
		};
	}, []);

	return (
		<ToastContainer
			newestOnTop
			position="bottom-right"
			transition={cssTransition({
				enter: 'animate-fade animate-duration-75',
				exit: 'animate-fade animate-duration-75 animate-reverse',
			})}
		/>
	);
}

function ToastIcon({
	isLoading,
	type,
}: {
	isLoading: boolean | undefined;
	type: string;
}) {
	if (isLoading)
		return <Spinner size="extraSmall" />;

	switch (type) {
		case 'success':
			return <CheckCircleIcon className="w-6 h-6 text-success" />;
		case 'warning':
			return <AlertTriangleIcon className="w-6 h-6 text-code-warn" />;
		case 'error':
			return <AlertCircleIcon className="w-6 h-6 text-danger" />;
		case 'info':
		default:
			return <InfoCircleIcon className="w-6 h-6 text-code-info" />;
	}
}
