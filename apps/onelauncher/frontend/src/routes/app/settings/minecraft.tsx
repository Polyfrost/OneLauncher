import { createFileRoute } from '@tanstack/react-router'
import Sidebar from './route'
import SettingsRow from '@/components/SettingsRow'
import { Database01Icon, EyeIcon, FilePlus02Icon, FileX02Icon, LayoutTopIcon, Maximize01Icon, ParagraphWrapIcon, XIcon } from '@untitled-theme/icons-react'
import Button from '@/components/base/Button'
import useCommand from '@/hooks/useCommand'
import { bindings } from '@/main'
import { TextField } from '@/components/base/TextField'
import ToggleButton from '@/components/base/ToggleButton'
import ScrollableContainer from '@/components/ScrollableContainer'
import Switch from '@/components/base/Switch'

export const Route = createFileRoute('/app/settings/minecraft')({
  component: RouteComponent,
})

function RouteComponent() {
  return (
    <Sidebar.Page>
      <ScrollableContainer>
        <div className="h-full">
          <h1>Minecraft Settings</h1>

          <GameSettings />

          {/* <LauncherSettings /> */}

          <ProcessSettings />
        </div>
      </ScrollableContainer>
    </Sidebar.Page>
  )
}

export function GameSettings() {
  return <>
    <SettingsRow.Header>Game</SettingsRow.Header>

    <SettingsRow
      description="Force Minecraft to start in fullscreen mode."
      icon={<Maximize01Icon />}
      title="Force Fullscreen"
    >
      <Switch />
    </SettingsRow>

    <SettingsRow
      description="The game window resolution in pixels."
      icon={<LayoutTopIcon />}
      title="Resolution"
    >
      <div className="grid grid-cols-[70px_16px_70px] gap-2 grid-justify-center grid-items-center">
        <TextField
          type='number'
          className="text-center"
        />
        <XIcon className="size-4 self-center" />
        <TextField
          type='number'
          className="text-center"
        />
      </div>
    </SettingsRow>

    <SettingsRow
      description="The amount of memory in megabytes allocated for the game."
      icon={<Database01Icon />}
      title="Memory"
    >
      <div className="flex items-center gap-x-4 flex-justify-center">
        <div className="flex flex-row items-center gap-x-2">
          <span>Min:</span>
          <TextField
            className="text-center"
            type='number'
            max={2024}
            min={1}
          />
        </div>

        <div className="flex flex-row items-center gap-x-2">
          <span>Max:</span>
          <TextField
            className="text-center"
            type='number'
            max={2024}
            min={1}
          />
        </div>
      </div>
    </SettingsRow>
  </>
}

function LauncherSettings() {
  return <>
    <SettingsRow.Header>Launcher</SettingsRow.Header>
    <SettingsRow
      description="Hide the launcher whenever you start a game."
      icon={<EyeIcon />}
      title="Hide On Launch"
    >
      <ToggleButton color='primary' children="false" />
    </SettingsRow>
  </>
}

export function ProcessSettings() {
  return <>
    <SettingsRow.Header>Process</SettingsRow.Header>

    <SettingsRow
      description="Command to run before launching the game."
      icon={<FilePlus02Icon />}
      title="Pre-Launch Command"
    >
      <TextField
        placeholder="echo 'Game started'"
      />
    </SettingsRow>

    <SettingsRow
      description="Command to run when launching the game."
      icon={<ParagraphWrapIcon />}
      title="Wrapper Command"
    >
      <TextField
        placeholder="gamescope"
      />
    </SettingsRow>

    <SettingsRow
      description="Command to run after exiting the game."
      icon={<FileX02Icon />}
      title="Post-Exit Command"
    >
      <TextField
        placeholder="echo 'Game exited'"
      />
    </SettingsRow>
  </>
}