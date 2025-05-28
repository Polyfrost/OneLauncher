import { createFileRoute, Outlet } from '@tanstack/react-router'
import Sidebar from '../settings/route'
import { EyeIcon, File06Icon, Globe04Icon, Image03Icon, PackagePlusIcon, Settings04Icon } from '@untitled-theme/icons-react'
import useCommand from '@/hooks/useCommand'
import { bindings } from '@/main'

type ClusterSearch = {
  id: bigint
}

export const Route = createFileRoute('/app/cluster')({
  component: RouteComponent,
  validateSearch: (search: Record<string, unknown>): ClusterSearch => {
    return {
      id: BigInt(search.id as string),
    }
  },
})

function RouteComponent() {
  const { id } = Route.useSearch()

  const cluster = useCommand("getClusterById", () => bindings.commands.getClusterById(Number(id.toString()) as unknown as bigint))

  const isModded = cluster.data?.mc_loader != "vanilla"

  return (
    <div className="h-full flex flex-row overflow-hidden">
      <div className="flex-shrink-0 w-72 flex flex-col pt-8 pr-7 pb-8">
        <Sidebar
          base="/app/cluster"
          links={{
            'Cluster Settings': [
              // if someone has a better way to do this, please let me know
              [<EyeIcon />, 'Overview', '/?id=' + id],
              (isModded ? [<PackagePlusIcon />, 'Mods', '/mods?id=' + id] : undefined),
              [<Image03Icon />, 'Screenshots', '/screenshots?id=' + id],
              [<Globe04Icon />, 'Worlds', '/worlds?id=' + id],
              [<File06Icon />, 'Logs', '/logs?id=' + id],
              [<Settings04Icon />, 'Game Settings', '/settings?id=' + id],
            ],
          }}
        />
        {/* <Info /> */}
      </div>

      <div className="flex-1 min-w-0 h-full">
        <Outlet />
      </div>
    </div>
  )
}
