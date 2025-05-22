import { type MinecraftCredentials } from '@/bindings.gen'
import Button from '@/components/base/Button'
import { Show } from '@/components/base/Show'
import PlayerHead from '@/components/content/PlayerHead'
import useCommand from '@/hooks/useCommand'
import { bindings } from '@/main'
import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'

export const Route = createFileRoute('/onboarding/login')({
  component: RouteComponent,
})

function RouteComponent() {
  const [profile, setProfile] = useState<MinecraftCredentials>()
  const [errorMessage, setErrorMessage] = useState<string>('')
  const result = useCommand("beginMsFlow", bindings.commands.beginMsFlow, {
    enabled: false,
    subscribed: false
  })

  function beginMsAuthFlow() {
    result.refetch()

    if (result.isError) {
      setErrorMessage(result.error.message)
      return;
    }

    if (!result.data) {
      setErrorMessage('No account was found. Please try again.');
      return;
    }

    setProfile(result.data);
  }

  return (
    <>
    {/* trust me i'll fix ui issues later */}
      <div className="grid grid-cols-2 h-full w-full flex-col items-start justify-center gap-x-16 gap-y-4">
        <h1 className="text-6xl -mb-2">Login</h1>

        <h3>Before you continue, we require you to own a copy of Minecraft: Java Edition.</h3>

        <Show when={profile}>
          <div className="w-full flex flex-row items-center justify-start gap-x-3 border border-border/05 rounded-lg bg-component-bg p-3">
            <PlayerHead className="rounded-md" uuid={profile?.id} />
            <div className="flex flex-col gap-y-2">
              <p className="text-lg text-fg-primary">{profile?.username}</p>
              <p className="text-fg-secondary">{profile?.id}</p>
            </div>
          </div>
        </Show>

        <Button
          children="Login with Microsoft"
          onPress={beginMsAuthFlow}
        />

        <p className="text-danger">{errorMessage}</p>

      </div>
    </>
  )
}
