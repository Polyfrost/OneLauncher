import SteveSkin from '@onelauncher/common/assets/skin/steve.png';

export function getSkinUrl(skinUrl: string | undefined | null) {
	return skinUrl || SteveSkin;
}
