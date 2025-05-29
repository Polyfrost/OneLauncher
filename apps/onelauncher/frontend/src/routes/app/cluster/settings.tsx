import { createFileRoute } from '@tanstack/react-router'
import Sidebar from '../settings/route'
import ScrollableContainer from '@/components/ScrollableContainer'
import useCommand from '@/hooks/useCommand'
import { bindings } from '@/main'
import { GameSettings, ProcessSettings } from '../settings/minecraft'

export const Route = createFileRoute('/app/cluster/settings')({
  component: RouteComponent,
})

function RouteComponent() {
  const { id } = Route.useSearch()

  const cluster = useCommand("getClusterById", () => bindings.commands.getClusterById(Number(id.toString()) as unknown as bigint))
  const result = useCommand("getProfileOrDefault", () => bindings.commands.getProfileOrDefault(cluster.data?.name as string), {
    enabled: !!cluster.data?.name
  })

  console.log(result.data)

  return (
    <Sidebar.Page>
      <ScrollableContainer>
        <div className="h-full">
          <h1>Minecraft Settings</h1>

          <GameSettings />

          <ProcessSettings />
        </div>
      </ScrollableContainer>
    </Sidebar.Page>
  )
}
