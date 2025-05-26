import { createFileRoute, Outlet } from '@tanstack/react-router'
import Sidebar from '../settings/route'
import { EyeIcon, File06Icon, Globe04Icon, Image03Icon, PackagePlusIcon, Settings04Icon } from '@untitled-theme/icons-react'

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
  return (
    <div className="h-full flex flex-row overflow-hidden">
      <div className="flex-shrink-0 w-72 flex flex-col pt-8 pr-7 pb-8">
        <Sidebar
          base="/app/cluster"
          links={{
            'Cluster Settings': [
              [<EyeIcon />, 'Overview', '/'],
              [<PackagePlusIcon />, 'Mods', '/mods'],
              [<Image03Icon />, 'Screenshots', '/screenshots'],
              [<Globe04Icon />, 'Worlds', '/worlds'],
              [<File06Icon />, 'Logs', '/logs'],
              [<Settings04Icon />, 'Game Settings', '/settings'],
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
