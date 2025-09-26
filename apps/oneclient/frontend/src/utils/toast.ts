import type { UpdateOptions } from 'react-toastify';
import { Toast } from '@/components/overlay';
import { toast as toastify } from 'react-toastify';

export type ToastId = string | number;

export interface ToastData {
	type: 'info' | 'success' | 'warning' | 'error';
	title: string;
	message?: string | undefined;
}

export interface ToastOptions extends ToastData {
	progress?: number;
	isLoading?: boolean | undefined;
	autoClose?: number | false | undefined;
}

export function toast({
	progress,
	isLoading,
	autoClose = 5000,
	...data
}: ToastOptions): ToastId {
	return toastify(Toast, {
		data,
		progress,
		isLoading,
		autoClose,
		closeOnClick: false,
		closeButton: false,
		hideProgressBar: true,
		icon: false,
	});
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
