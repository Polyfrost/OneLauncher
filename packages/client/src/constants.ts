export const WEBSITE = 'https://polyfrost.org/';

// TODO: temporary
export const PROGRAM_INFO = { arch: '64', dev_build: true, launcher_version: '1.0.0-alpha.1', platform: 'osx', tauri_version: '2.0.0-rc.0', webview_version: '19618.3.11.11.5' } as const;
export type ProgramInfo = typeof PROGRAM_INFO;
