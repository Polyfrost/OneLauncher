import type { MinecraftCredentials } from '@/bindings.gen';
import { Button } from '@onelauncher/common/components';
import { useNavigate } from '@tanstack/react-router';
import { Pencil01Icon } from '@untitled-theme/icons-react';
import { useCallback } from 'react';

export function ManageSkinButton({ profile }: { profile: MinecraftCredentials }) {
	const navigate = useNavigate();
	const manageSkin = useCallback(() => navigate({ to: `/app/account/skins`, search: { profile } }), [profile, navigate]);

	return (
		<Button
			className="group w-8 h-8"
			color="ghost"
			onPress={manageSkin}
			size="icon"
		>
			<Pencil01Icon className="group-hover:stroke-brand-hover" />
		</Button>
	);
}
