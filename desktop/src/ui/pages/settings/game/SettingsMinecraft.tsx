import { EyeIcon, LayoutTopIcon, Maximize01Icon, XIcon } from '@untitled-theme/icons-solid';
import { Show } from 'solid-js';
import TextField from '~ui/components/base/TextField';
import Toggle from '~ui/components/base/Toggle';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import SettingsRow from '~ui/components/SettingsRow';
import Sidebar from '~ui/components/Sidebar';
import useSettingsContext from '~ui/hooks/useSettings';

function SettingsMinecraft() {
	const settings = useSettingsContext();

	return (
		<Sidebar.Page>
			<h1>Global Minecraft Settings</h1>
			<ScrollableContainer>
				<SettingsMinecraft.Settings
					fullscreen={{
						get: settings.force_fullscreen ?? false,
						set: value => settings.force_fullscreen = value,
					}}
					resolution={{
						get: settings.resolution,
						set: value => settings.resolution = value,
					}}
					hide_on_launch={{
						get: settings.hide_on_launch ?? false,
						set: value => settings.hide_on_launch = value,
					}}
				/>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

interface SettingsEntryType<T> {
	get: T;
	set: (value: T) => any;
	isGlobal?: boolean; // TODO: Ask how "global settings sync" should work on pages like ClusterSettings
};

interface SettingsProps {
	fullscreen: SettingsEntryType<boolean>;
	resolution: SettingsEntryType<[number, number]>;
	hide_on_launch: SettingsEntryType<boolean>;
};

SettingsMinecraft.Settings = function (props: Partial<SettingsProps>) {
	return (
		<>
			<Show when={props.fullscreen !== undefined}>
				<SettingsRow
					title="Force Fullscreen"
					description="Force Minecraft to start in fullscreen mode."
					icon={<Maximize01Icon />}
				>
					<Toggle
						defaultChecked={props.fullscreen!.get}
						onChecked={props.fullscreen!.set}
					/>
				</SettingsRow>
			</Show>

			<Show when={props.resolution !== undefined}>
				<SettingsRow
					title="Resolution"
					description="The game window resolution in pixels."
					icon={<LayoutTopIcon />}
				>
					<div class="grid grid-justify-center grid-items-center gap-2 grid-cols-[70px_16px_70px]">
						<TextField.Number
							class="text-center"
							value={props.resolution!.get[0]}
							onValidSubmit={(value) => {
								props.resolution!.set([Number.parseInt(value), props.resolution!.get[1]]);
							}}
						/>
						<XIcon class="w-4 h-4" />
						<TextField.Number
							class="text-center"
							value={props.resolution!.get[1]}
							onValidSubmit={(value) => {
								props.resolution!.set([props.resolution!.get[0], Number.parseInt(value)]);
							}}
						/>
					</div>
				</SettingsRow>
			</Show>

			<Show when={props.hide_on_launch !== undefined}>
				<SettingsRow
					title="Hide On Launch"
					description="Hide the launcher whenever you start a game."
					icon={<EyeIcon />}
				>
					<Toggle
						defaultChecked={props.hide_on_launch!.get}
						onChecked={props.hide_on_launch!.set}
					/>
				</SettingsRow>
			</Show>
		</>
	);
};

export default SettingsMinecraft;
