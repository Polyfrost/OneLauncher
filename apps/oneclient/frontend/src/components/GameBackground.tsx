/* eslint-disable perfectionist/sort-imports -- nah */
import type { CSSProperties } from 'react';
import { twMerge } from 'tailwind-merge';

import CavesAndCliffs from '@/assets/backgrounds/CavesAndCliffs.jpg';
import HypixelSkyblockHub from '@/assets/backgrounds/HypixelSkyblockHub.png';

const GameBackgrounds = {
	HypixelSkyblockHub,
	CavesAndCliffs,
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
			className={twMerge(`pointer-events-none inset-0 bg-cover bg-center bg-no-repeat`, className)}
			style={{ backgroundImage: `url(${BackgroundImage})`, ...style }}
		/>
	);
}
