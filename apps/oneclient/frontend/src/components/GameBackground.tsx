import type { CSSProperties } from 'react';
import { twMerge } from 'tailwind-merge';
import HypixelSkyblockHub from '../assets/backgrounds/HypixelSkyblockHub.png';

const GameBackgrounds = {
	HypixelSkyblockHub,
};

export type GameBackgroundName = keyof typeof GameBackgrounds;

interface GameBackgroundProps {
	name: GameBackgroundName;
	className?: string;
	style?: CSSProperties;
}

export function GameBackground({ name, className = '', style = undefined }: GameBackgroundProps) {
	const BackgroundImage = GameBackgrounds[name];

	return (
		<div
			className={twMerge(`absolute -z-50 pointer-events-none inset-0 bg-cover bg-center`, className)}
			style={{ backgroundImage: `url(${BackgroundImage})`, ...style }}
		/>
	);
}
