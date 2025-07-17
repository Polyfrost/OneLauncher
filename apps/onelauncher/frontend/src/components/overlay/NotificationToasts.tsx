import type { NotificationData } from '@/hooks/useNotification';
import useNotifications from '@/hooks/useNotification';
import { Button, Show } from '@onelauncher/common/components';
import { InfoCircleIcon, XIcon } from '@untitled-theme/icons-react';
import { useCallback, useEffect, useState } from 'react';
import { twMerge } from 'tailwind-merge';

interface ToastNotification extends NotificationData {
	id: string;
	timestamp: number;
}

interface NotificationToastProps {
	notification: ToastNotification;
	onRemove: (id: string) => void;
}

const fractionEnded = (fraction: number | undefined) => fraction !== undefined && fraction >= 0.97;

export function NotificationToast({ notification, onRemove }: NotificationToastProps) {
	const [isVisible, setIsVisible] = useState(false);
	const [isRemoving, setIsRemoving] = useState(false);

	const handleRemove = useCallback(() => {
		setIsRemoving(true);
		setTimeout(() => {
			onRemove(notification.id);
		}, 300);
	}, [notification.id, onRemove]);

	useEffect(() => {
		// Trigger entrance animation
		const timer = setTimeout(() => setIsVisible(true), 50);
		return () => clearTimeout(timer);
	}, []);

	useEffect(() => {
		// Auto-remove after 5 seconds if no progress indicator
		if (notification.fraction === undefined) {
			const timer = setTimeout(handleRemove, 5000);
			return () => clearTimeout(timer);
		}
	}, [notification.fraction, handleRemove]);

	return (
		<div className={twMerge('flex flex-col gap-y-1 p-2 bg-component-bg border border-component-border rounded-lg shadow-lg', isVisible && !isRemoving ? 'translate-x-0 opacity-100' : 'translate-x-full opacity-0')}>
			<div className="grid grid-cols-[24px_1fr_auto] min-h-10 place-items-center gap-3">
				<InfoCircleIcon className="h-6 w-6 text-fg-primary" />

				<div className="w-full flex flex-col gap-y-1">
					<span className="text-fg-primary font-medium capitalize">{notification.title}</span>
					<span className="text-sm text-fg-secondary/60 capitalize">{notification.message}</span>
				</div>
				<Button
					className="flex-shrink-0 h-6 w-6 p-0"
					color="ghost"
					onClick={handleRemove}
					size="icon"
				>
					<XIcon className="h-3 w-3" />
				</Button>
			</div>

			<Show when={notification.fraction !== undefined && !fractionEnded(notification.fraction)}>
				<div className="h-1.5 w-full overflow-hidden rounded-full bg-brand-disabled/10">
					<div
						className="h-full max-w-full min-w-0 rounded-full bg-brand transition-width"
						style={{
							width: `${Math.floor(notification.fraction! * 100)}%`,
						}}
					/>
				</div>
			</Show>
		</div>
	);
}

function NotificationToasts() {
	const { list } = useNotifications();
	const [toasts, setToasts] = useState<Array<ToastNotification>>([]);

	useEffect(() => {
		const currentToasts = Object.entries(list).map(([id, data]) => ({
			id,
			...data,
			timestamp: Date.now(),
		}));

		setToasts((prev) => {
			const existingIds = new Set(prev.map(t => t.id));
			const newToasts = currentToasts.filter(toast => !existingIds.has(toast.id));

			const updatedToasts = prev.map((toast) => {
				const updated = currentToasts.find(t => t.id === toast.id);
				return updated ? { ...toast, ...updated } : toast;
			});

			const currentIds = new Set(currentToasts.map(t => t.id));
			const filteredToasts = updatedToasts.filter(toast => currentIds.has(toast.id));

			return [...filteredToasts, ...newToasts];
		});
	}, [list]);

	const removeToast = (id: string) => {
		setToasts(prev => prev.filter(toast => toast.id !== id));
	};

	if (toasts.length === 0)
		return null;

	return (
		<div className="fixed bottom-4 right-4 z-50 pointer-events-none">
			<div className="pointer-events-auto">
				{toasts.map(toast => (
					<NotificationToast
						key={toast.id}
						notification={toast}
						onRemove={removeToast}
					/>
				))}
			</div>
		</div>
	);
}

export default NotificationToasts;
