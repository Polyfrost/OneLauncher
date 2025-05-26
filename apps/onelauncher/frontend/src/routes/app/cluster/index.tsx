import useCommand from '@/hooks/useCommand'
import { bindings } from '@/main'
import { createFileRoute } from '@tanstack/react-router'
import Sidebar from '../settings/route'

export const Route = createFileRoute('/app/cluster/')({
  component: RouteComponent
})

function RouteComponent() {
  const { id } = Route.useSearch()

  // dumbass fix ik
  const cluster = useCommand("getClusterById", () => bindings.commands.getClusterById(Number(id.toString()) as unknown as bigint))

  return (
    <Sidebar.Page>
      <div className="h-full">
        <pre>{JSON.stringify(cluster, null, 2)}</pre>
      </div>
    </Sidebar.Page>
  )
}
