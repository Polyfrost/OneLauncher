import type { NotificationData } from '@/hooks/useNotification';
import useNotifications from '@/hooks/useNotification';
import { Button } from '@onelauncher/common/components';
import { XIcon } from '@untitled-theme/icons-react';
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

function NotificationToast({ notification, onRemove }: NotificationToastProps) {
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
		<div
			className={twMerge(
				'transform transition-all duration-300 ease-in-out',
				'bg-component-bg border border-component-border rounded-lg shadow-lg',
				'p-4 mb-3 max-w-sm',
				isVisible && !isRemoving ? 'translate-x-0 opacity-100' : 'translate-x-full opacity-0',
			)}
		>
			<div className="flex items-start justify-between gap-3">
				<div className="flex-1 min-w-0">
					<h4 className="font-semibold text-fg-primary text-sm truncate">
						{notification.title}
					</h4>
					<p className="text-fg-secondary text-xs mt-1 break-words">
						{notification.message}
					</p>
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

			{notification.fraction !== undefined && (
				<div className="w-full bg-component-bg-hover rounded-full h-2 mt-3">
					<div
						className="bg-brand h-2 rounded-full transition-all duration-300"
						style={{ width: `${Math.max(0, Math.min(100, notification.fraction))}%` }}
					/>
				</div>
			)}
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
