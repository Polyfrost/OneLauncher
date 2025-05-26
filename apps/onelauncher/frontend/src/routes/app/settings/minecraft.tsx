import { createFileRoute } from '@tanstack/react-router'
import Sidebar from './route'
import SettingsRow from '@/components/SettingsRow'
import { Maximize01Icon } from '@untitled-theme/icons-react'
import Button from '@/components/base/Button'

export const Route = createFileRoute('/app/settings/minecraft')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <Sidebar.Page>
      <div className="h-full">
        <h1>Minecraft Settings</h1>

        <SettingsRow.Header>Game</SettingsRow.Header>

        <SettingsRow
          description="Force Minecraft to start in fullscreen mode."
          icon={<Maximize01Icon />}
          title="Force Fullscreen"
        >
          <Button>something</Button>
        </SettingsRow>
      </div>
    </Sidebar.Page>
  )
}
