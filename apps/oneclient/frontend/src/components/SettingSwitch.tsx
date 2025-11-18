import { Switch } from '@onelauncher/common/components';

type BooleanSetting = [
	boolean,
	(value: boolean) => void,
];

export function SettingsSwitch({ setting }: { setting: BooleanSetting }) {
	return (
		<Switch
			isSelected={setting[0]}
			onChange={val => setting[1](val)}
		/>
	);
}
