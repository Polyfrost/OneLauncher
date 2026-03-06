import type { ToastData, ToastOptions } from '@/utils/toast';
import { MinecraftAuthErrorModal, minecraftAuthErrors, Overlay, RawDebugInfo, SettingNumber, SettingsRow, SettingSwitch, SheetPage } from '@/components';
import { useDebugInfo } from '@/hooks/useDebugInfo';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { TitleCase } from '@/utils/string';
import { ToastPositions, ToastTypes, useToast } from '@/utils/toast';
import { Button, Dropdown, TextField } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { dataDir, join } from '@tauri-apps/api/path';
import { useEffect, useState } from 'react';

export const Route = createFileRoute('/app/debug')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<SheetPage
			headerLarge={<HeaderLarge />}
			headerSmall={<HeaderSmall />}
		>
			<SheetPage.Content>
				<div className="flex flex-col gap-4">

					<AuthError />
					<div className="w-full h-1 my-2 rounded bg-component-border"></div>
					<Toasts />
					<div className="w-full h-1 my-2 rounded bg-component-border"></div>
					<Settings />
					<div className="w-full h-1 my-2 rounded bg-component-border"></div>
					<Other />
					<div className="w-full h-1 my-2 rounded bg-component-border"></div>
					<Info />

				</div>

			</SheetPage.Content>
		</SheetPage>
	);
}

function AuthError() {
	const [errorInput, setErrorInput] = useState<string>('Some unknown auth error occurred');
	const [previewError, setPreviewError] = useState<string | null>(null);

	return (
		<>
			<div className="flex flex-col gap-4">
				<h1 className="text-3xl font-semibold">Auth Error Preview</h1>
				<div className="flex flex-row gap-4 items-center">
					<p>Custom Input:</p>
					<TextField className="flex-1" onChange={e => setErrorInput(e.target.value)} value={errorInput} />
				</div>
				<div className="flex flex-row flex-wrap gap-4">
					{minecraftAuthErrors.map(err => (
						<Button
							key={err.errorCode}
							onClick={() => setPreviewError(`Minecraft authentication error: ${err.errorCode} during MSA step XstsAuthorize`)}
							size="normal"
						>
							{err.errorCode}
						</Button>
					))}
					<Button onClick={() => setPreviewError(errorInput)} size="normal">Custom</Button>
				</div>
			</div>

			{previewError && (
				<Overlay
					isDismissable
					isOpen
					onOpenChange={(open) => {
						if (!open)
							setPreviewError(null);
					}}
				>
					<MinecraftAuthErrorModal error={previewError} />
				</Overlay>
			)}
		</>
	);
}

function Toasts() {
	const [title, setTitle] = useState<string>('Title');
	const [message, setMessage] = useState<string | undefined>('Message');
	const [type, setType] = useState<ToastData['type']>('info');
	const [position, setPosition] = useState<ToastOptions['position'] | 'undefined'>('undefined');
	const [duration, setDuration] = useState<number>(5000);
	const [autoClose, setAutoClose] = useState<boolean>(true);
	const toast = useToast();

	const sendToast = () => toast({
		type,
		title,
		message,
		position: position === 'undefined' ? undefined : position,
		autoClose: autoClose ? duration : false,
	});

	return (
		<div className="flex flex-col gap-4">
			<h1 className="text-3xl font-semibold">Toasts</h1>
			<div className="flex flex-row gap-4 items-center">
				<p>Title:</p>
				<TextField className="flex-1" onChange={e => setTitle(e.target.value)} value={title} />
			</div>
			<div className="flex flex-row gap-4 items-center">
				<p>Message:</p>
				<TextField className="flex-1" onChange={e => setMessage(e.target.value)} value={message} />
			</div>

			<div className="flex flex-row gap-4 items-center">
				<p>Type:</p>
				<Dropdown onSelectionChange={key => setType(key as ToastData['type'])} selectedKey={type}>
					{ToastTypes.map(type => <Dropdown.Item id={type} key={type}>{TitleCase(type)}</Dropdown.Item>)}
				</Dropdown>
			</div>
			<div className="flex flex-row gap-4 items-center">
				<p>Position:</p>
				<Dropdown onSelectionChange={key => setPosition(key as ToastOptions['position'])} selectedKey={position}>
					<Dropdown.Item id="undefined" key="undefined">Config</Dropdown.Item>
					{ToastPositions.map(type => <Dropdown.Item id={type} key={type}>{TitleCase(type)}</Dropdown.Item>)}
				</Dropdown>
			</div>
			<div className="flex flex-row gap-4 items-center">
				<p>Duration:</p>
				<SettingNumber max={60000} min={500} setting={[duration, (value: number) => setDuration(value)]} />
			</div>
			<div className="flex flex-row gap-4 items-center">
				<p>Auto Close:</p>
				<SettingSwitch setting={[autoClose, (value: boolean) => setAutoClose(value)]} />
			</div>

			<Button onPress={sendToast} size="normal">Send Toast</Button>
		</div>
	);
}

function Settings() {
	const { createSetting } = useSettings();

	return (
		<div className="flex flex-col gap-4">
			<h1 className="text-3xl font-semibold">Settings</h1>
			<div className="flex flex-row gap-4 flex-wrap">

				<SettingsRow description="WARNING! This requires a restart to apply. Logs out debug info" title="Log Debug Info">
					<SettingSwitch setting={createSetting('log_debug_info')} />
				</SettingsRow>

				<SettingsRow description="Enable The Tanstack Dev Tools and shows debug page" title="Show Dev stuff">
					<SettingSwitch setting={createSetting('show_tanstack_dev_tools')} />
				</SettingsRow>

				<SettingsRow description="Seen onboarding" title="Seen Onboarding">
					<SettingSwitch setting={createSetting('seen_onboarding')} />
				</SettingsRow>

				<SettingsRow description="Use Grid On Mods List" title="Use Grid On Mods List">
					<SettingSwitch setting={createSetting('mod_list_use_grid')} />
				</SettingsRow>
			</div>
		</div>
	);
}

function Other() {
	const [launcherDir, setLauncherDir] = useState('');
	useEffect(() => {
		(async () => {
			setLauncherDir(await join(await dataDir(), 'OneClient'));
		})();
	}, []);

	const openLauncherDir = () => bindings.core.open(launcherDir);

	const logRunningProcesses = () => {
		(async () => {
			const running = await bindings.core.getRunningProcesses();
			// eslint-disable-next-line no-console -- Designed to log
			console.log(running);
		})();
	};

	return (
		<div className="flex flex-col gap-4">
			<h1 className="text-3xl font-semibold">Other</h1>
			<div className="flex flex-row gap-4">
				<Button onPress={bindings.debug.openDevTools} size="normal">Open Dev Tools</Button>
				<Button onClick={openLauncherDir} size="normal">Open Launcher Data</Button>
				<Button onClick={logRunningProcesses} size="normal">Log Running Processes to frontend</Button>
			</div>
		</div>
	);
}

function Info() {
	const debugInfo = useDebugInfo();

	return (
		<div className="flex flex-col gap-4">
			<h1 className="text-3xl font-semibold">Info</h1>
			<div className="flex flex-row gap-4">
				<RawDebugInfo debugInfo={debugInfo} />
			</div>
		</div>
	);
}

function HeaderLarge() {
	return (
		<div className="flex flex-row justify-between items-end gap-16">
			<div className="flex-1 flex flex-col">
				<h1 className="text-3xl font-semibold">Debug</h1>
				<p>The end user won't really be looking at this page. Design skills go out the window</p>
			</div>
		</div>
	);
}

function HeaderSmall() {
	return (
		<div className="flex flex-row justify-between items-center h-full">
			<h1 className="text-2lg h-full font-medium">Debug</h1>
		</div>
	);
}
