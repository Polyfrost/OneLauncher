declare namespace Core {
    export interface Notification {
        id: number;
        created_at: number;
        title: string;
        message: string;
        notification_type: NotificationType;
        progress?: number;
    }

    export type NotificationEvents = 'init' | 'added' | 'removed' | 'cleared';
}