import { createFileRoute } from '@tanstack/react-router'
import Sidebar from './route'
import useCommand from '@/hooks/useCommand'
import { bindings } from '@/main'
import SettingsRow from '@/components/SettingsRow'
import DiscordIcon from "@/assets/logos/discord.svg"
import ToggleButton from '@/components/base/ToggleButton'
import Switch from '@/components/base/Switch'

export const Route = createFileRoute('/app/settings/')({
    component: RouteComponent,
})

function RouteComponent() {
    const result = useCommand("getGlobalProfile", bindings.commands.getGlobalProfile)

    return (
        <Sidebar.Page>
            <div className="h-full">
                <h1>General Settings</h1>

                <SettingsRow
					description="Enable Discord Rich Presence."
					icon={<img src={DiscordIcon} className="w-6 invert-100" />}
					title="Discord RPC"
				>
					<Switch>Thing</Switch>
				</SettingsRow>
                
                <pre>{JSON.stringify(result.data, null, 2)}</pre>
            </div>
        </Sidebar.Page>
    )
}
