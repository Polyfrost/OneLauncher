declare namespace Core {
    export interface Notification {
        created_at: number;
        title: string;
        message: string;
        notification_type: NotificationType;
        progress?: number;
    }

    export type NotificationEvents = 'added' | 'removed' | 'cleared';
}