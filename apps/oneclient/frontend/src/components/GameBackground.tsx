import { twMerge } from 'tailwind-merge';
import HypixelSkyblockHub from '../assets/backgrounds/HypixelSkyblockHub.png';

const GameBackgrounds = {
	HypixelSkyblockHub,
};

type GameBackgroundName = keyof typeof GameBackgrounds;

interface GameBackgroundProps {
	name: GameBackgroundName;
	className?: string;
}

export function GameBackground({ name, className = '' }: GameBackgroundProps) {
	const BackgroundImage = GameBackgrounds[name];

	return (
		<div
			className={twMerge(`absolute -z-50 pointer-events-none inset-0 bg-cover bg-center`, className)}
			style={{ backgroundImage: `url(${BackgroundImage})` }}
		/>
	);
}
