export interface AppError {
	data: string | AppError;
	type: string;
}

export function isLauncherError<TErr = AppError>(error: unknown): error is TErr {
	return typeof error === 'object'
		&& error !== null
		&& 'data' in error
		&& 'type' in error;
}

export function getMessageFromError<T extends AppError>(error: T): string;
export function getMessageFromError<T extends AppError>(error: T | null | undefined): string | null;
export function getMessageFromError(error: AppError | null | undefined): string | null {
	if (!error)
		return null;

	if (typeof error.data === 'string')
		return error.data;

	return getMessageFromError(error.data);
}
