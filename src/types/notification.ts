// src/types/notification.ts
export type NotificationType = 'error' | 'warning' | 'info' | 'success';

export interface NotificationPayload {
  type: NotificationType;
  message: string;
  /** Raw error object for full context chain logging */
  error?: unknown;
}

export type OnNotify = (payload: NotificationPayload) => void;

