import { createFileRoute } from '@tanstack/react-router'
import Sidebar from './route'
import useCommand from '@/hooks/useCommand'
import { bindings } from '@/main'
import SettingsRow from '@/components/SettingsRow'
import DiscordIcon from "@/assets/logos/discord.svg"
import Switch from '@/components/base/Switch'
import Button from '@/components/base/Button'
import { FolderIcon, LinkExternal01Icon, XIcon } from '@untitled-theme/icons-react'

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
                    <Switch />
                </SettingsRow>

                <SettingsRow
                    description="Hide the confirmation dialog when closing the launcher."
                    icon={<XIcon />}
                    title="Hide Close Dialog"
                >
                    <Switch />
                </SettingsRow>

                {/* <SettingsRow
					description="Sends errors and crash logs using Sentry to help developers fix issues. (// TODO)"
					icon={<AlertSquareIcon />}
					title="Error Analytics"
				>
					<Toggle
						checked={() => !(settings().disable_analytics ?? false)}
						onChecked={value => settings().disable_analytics = !value}
					/>
				</SettingsRow> */}

                <SettingsRow.Header>Folders and Files</SettingsRow.Header>
                <SettingsRow
                    description={'Unknown for now'}
                    icon={<FolderIcon />}
                    title="Launcher Folder"
                >
                    <Button size='normal'>
                        <LinkExternal01Icon /> Open
                    </Button>
                </SettingsRow>
            </div>
        </Sidebar.Page>
    )
}
