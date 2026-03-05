import type { ToastPosition } from '@/bindings.gen';
import type { UpdateOptions } from 'react-toastify';
import { Toast } from '@/components/overlay';
import { useSettings } from '@/hooks/useSettings';
import { toast as toastify } from 'react-toastify';

export type ToastId = string | number;

export interface ToastData {
	type: 'info' | 'success' | 'warning' | 'error';
	title: string;
	message?: string | undefined;
}
export const ToastTypes: Array<ToastData['type']> = ['info', 'success', 'warning', 'error'];

export interface ToastOptions extends ToastData {
	progress?: number;
	isLoading?: boolean | undefined;
	autoClose?: number | false | undefined;
	position?: ToastPosition;
}
export const ToastPositions: Array<NonNullable<ToastOptions['position']>> = ['top-right', 'top-left', 'bottom-right', 'bottom-left'];

export function useToast() {
	const { setting } = useSettings();

	return function toast({
		progress,
		isLoading,
		autoClose,
		position,
		...data
	}: ToastOptions): ToastId {
		if (position === undefined)
			position = setting('toast_position');

		if (autoClose === undefined)
			autoClose = setting('toast_duration');

		return toastify(Toast, {
			data,
			progress,
			isLoading,
			autoClose,
			closeOnClick: false,
			closeButton: false,
			hideProgressBar: true,
			icon: false,
			position,
		});
	};
}

export function toastUpdate(
	toastId: ToastId,
	{
		progress,
		isLoading,
		autoClose,
		...data
	}: Partial<ToastOptions>,
) {
	const objData: UpdateOptions<Record<string, unknown>> = {
		progress,
		isLoading,
	};

	for (const [key, value] of Object.entries<unknown>(data)) {
		if (value === undefined || value === null)
			continue;

		if (!objData.data)
			objData.data = {};

		objData.data[key] = value;
	}

	toastify.update(toastId, objData);
}
