$LASTEXITCODE = 0
$ErrorActionPreference = if ($env:CI) { 'Stop' } else { 'Inquire' }
Set-StrictMode -Version Latest

function Reset-Path {
    $env:Path = [System.Environment]::ExpandEnvironmentVariables(
        [System.Environment]::GetEnvironmentVariable('Path', 'Machine') +
        [IO.Path]::PathSeparator +
        [System.Environment]::GetEnvironmentVariable('Path', 'User')
    )
}

# verifies if the environment is Windows 64-bit and if the user is an administrator
if ((-not [string]::IsNullOrEmpty($env:PROCESSOR_ARCHITEW6432)) -or (
        "$env:PROCESSOR_ARCHITECTURE" -eq 'ARM64'
    ) -or (
        -not [System.Environment]::Is64BitOperatingSystem
        # powershell >= 6 is cross-platform, check if we are running on windows
    ) -or (($PSVersionTable.PSVersion.Major -ge 6) -and (-not $IsWindows))
) {
    $ErrorActionPreference = 'Continue'
    Write-Host
    if (Test-Path "$($env:WINDIR)\SysNative\WindowsPowerShell\v1.0\powershell.exe" -PathType Leaf) {
        throw 'You are using PowerShell (32-bit), please re-run in PowerShell (64-bit)'
    } else {
        throw 'This script is only supported on Windows 64-bit operating systems, use the script.sh for Unix systems.'
    }
    Exit 1
} elseif (
    -not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
) {
    # starts a new powershell process with administrator privileges and set the working directory to the directory where the script is located
    $proc = Start-Process -PassThru -Wait -FilePath 'PowerShell.exe' -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$($MyInvocation.MyCommand.Definition)`"" -WorkingDirectory "$PSScriptRoot"
    # resets path so the user doesn't have to restart the shell to use the tools installed by this script
    Reset-Path
    Exit $proc.ExitCode
}

# exits the script with an error
function Exit-WithError($err, $help = $null) {
    if ($null -ne $help) {
        Write-Host
        Write-Host $help -ForegroundColor DarkRed
    }
    throw $err
    Exit 1
}

# adds a directory to the path env variable
function Add-DirectoryToPath($directory) {
    Reset-Path
    if ($env:Path.Split([IO.Path]::PathSeparator) -notcontains $directory) {
        [System.Environment]::SetEnvironmentVariable(
            'Path',
            [System.Environment]::GetEnvironmentVariable('Path', 'User') + [IO.Path]::PathSeparator + $directory,
            'User'
        )

        if ($env:CI) {
            # if running in CI, we need to use GITHUB_PATH instead of the normal PATH env variables
            Add-Content $env:GITHUB_PATH "$directory`n"
        }
    }
    Reset-Path
}

# resets PATH to ensure the script doesn't have stale PATH entries
Reset-Path

# gets project dir (get dir from script location: <projectRoot>\packages\scripts\setup.ps1)
# Determine if the script is run from the project root directory
$currentDir = Get-Location
$scriptDir = Get-Item -Path $PSScriptRoot

if ($currentDir.FullName -eq $scriptDir.Parent.Parent.FullName) {
    $projectRoot = $currentDir.FullName
} else {
    $projectRoot = $scriptDir.Parent.Parent.FullName
}

$packageJsonPath = "$projectRoot\package.json"
$packageJson = Get-Content -Raw -Path $packageJsonPath | ConvertFrom-Json
$wingetValidExit = 0, -1978335189, -1978335153, -1978335135

Write-Host 'OneLauncher Development Environment Setup' -ForegroundColor Magenta
Write-Host @"

To set up your machine for OneLauncher development, this script will do the following:
1) Install Windows C++ build tools
2) Install Edge Webview 2
3) Install Rust and Cargo
4) Install Rust tools (if not already installed)
5) Install Node.js, npm and pnpm (if not already installed)
"@

# install system dependencies (github actions already has all of these installed)
if (-not $env:CI) {
    if (-not (Get-Command winget -ea 0)) {
        Exit-WithError 'winget not available' @'
Follow the instructions here to install winget:
https://learn.microsoft.com/windows/package-manager/winget/
'@
    }

    # check that system winget version is greater or equal to v1.4.10052
    $wingetVersion = [Version]((winget --version) -replace '.*?(\d+)\.(\d+)\.(\d+).*', '$1.$2.$3')
    $requiredVersion = [Version]'1.4.10052'
    if ($wingetVersion.CompareTo($requiredVersion) -lt 0) {
        $errorMessage = "You need to update your winget to version $requiredVersion or higher."
        Exit-WithError $errorMessage
    }

    # check for connectivity to github
    $ProgressPreference = 'SilentlyContinue'
    if (-not ((Test-NetConnection -ComputerName 'github.com' -Port 80).TcpTestSucceeded)) {
        Exit-WithError "Can't connect to github, check your internet connection and run this script again"
    }
    $ProgressPreference = 'Continue'

    Write-Host
    Read-Host 'Press Enter to continue'

    # TODO: force update visual studio build tools
    Write-Host
    Write-Host 'Installing Visual Studio Build Tools...' -ForegroundColor Yellow
    Write-Host 'This will take some time as it involves downloading several gigabytes of data....' -ForegroundColor Cyan
    winget install -e --accept-source-agreements --force --disable-interactivity --id Microsoft.VisualStudio.2022.BuildTools `
        --override 'updateall --quiet --wait'
    # force install because buildtools is itself a package manager, so let it decide if something needs to be installed or not
    winget install -e --accept-source-agreements --force --disable-interactivity --id Microsoft.VisualStudio.2022.BuildTools `
        --override '--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
    if (-not ($wingetValidExit -contains $LASTEXITCODE)) {
        Exit-WithError 'Failed to install Visual Studio Build Tools'
    } else {
        $LASTEXITCODE = 0
    }

    Write-Host
    Write-Host 'Installing Edge Webview 2...' -ForegroundColor Yellow
    # this is normally already available, but on some early windows 10 versions it isn't
    winget install -e --accept-source-agreements --disable-interactivity --id Microsoft.EdgeWebView2Runtime
    if (-not ($wingetValidExit -contains $LASTEXITCODE)) {
        Exit-WithError 'Failed to install Edge Webview 2'
    } else {
        $LASTEXITCODE = 0
    }

    Write-Host
    Write-Host 'Installing Rust and Cargo...' -ForegroundColor Yellow
    winget install -e --accept-source-agreements --disable-interactivity --id Rustlang.Rustup
    if (-not ($wingetValidExit -contains $LASTEXITCODE)) {
        Exit-WithError 'Failed to install Rust and Cargo'
    } else {
        $LASTEXITCODE = 0
    }

    Write-Host
    Write-Host 'Installing Nasm...' -ForegroundColor Yellow
    winget install -e --accept-source-agreements --disable-interactivity --id NASM.NASM
    if (-not ($wingetValidExit -contains $LASTEXITCODE)) {
        Exit-WithError 'Failed to install Nasm'
    } else {
        $LASTEXITCODE = 0
    }

    Write-Host
    Write-Host 'Installing NodeJS...' -ForegroundColor Yellow
    # check if nodejs is already installed and if it's compatible with the project
    $currentNode = Get-Command node -ea 0
    $currentNodeVersion = if (-not $currentNode) { $null } elseif ($currentNode.Version) { $currentNode.Version } elseif ((node --version) -match '(?sm)(\d+(\.\d+)*)') { [Version]$matches[1] } else { $null }
    $enginesNodeVersion = if ($packageJson.engines.node -match '(?sm)(\d+(\.\d+)*)') { [Version]$matches[1] } else { $null }
    if ($currentNodeVersion -and $enginesNodeVersion -and $currentNodeVersion.CompareTo($enginesNodeVersion) -lt 0) {
        Exit-WithError "Current Node.JS version: $currentNodeVersion (required: $enginesNodeVersion)" `
            'Uninstall the current version of Node.JS and run this script again'
    }

    # installs nodejs
    winget install -e --accept-source-agreements --disable-interactivity --id OpenJS.NodeJS
    if (-not ($wingetValidExit -contains $LASTEXITCODE)) {
        Exit-WithError 'Failed to install NodeJS'
    } else {
        $LASTEXITCODE = 0
    }

    # adds nodejs to the PATH
    Add-DirectoryToPath "$env:SystemDrive\Program Files\nodejs"

    # rests PATH to ensure that executable installed above are available to rest of the script
    Reset-Path

    Write-Host
    Write-Host 'Installing Rust MSVC Toolchain...' -ForegroundColor Yellow
    rustup toolchain install stable-msvc
    if ($LASTEXITCODE -ne 0) {
        Exit-WithError 'Failed to install Rust MSVC Toolchain'
    }

    Write-Host
    Write-Host 'Installing Rust tools...' -ForegroundColor Yellow
    cargo install cargo-watch
    if ($LASTEXITCODE -ne 0) {
        Exit-WithError 'Failed to install Rust tools'
    }

    Write-Host
    Write-Host 'Installing for pnpm...' -ForegroundColor Yellow
    # checks if pnpm is already installed and if it's compatible with the project
    $currentPnpmVersion = if (-not (Get-Command pnpm -ea 0)) { $null } elseif ((pnpm --version) -match '(?sm)(\d+(\.\d+)*)') { [Version]$matches[1] } else { $null }
    $enginesPnpmVersion = if ($packageJson.engines.pnpm -match '(?sm)(\d+(\.\d+)*)') { [Version]$matches[1] } else { $null }

    if (-not $currentPnpmVersion) {
        # removes possible remaining envvars from old pnpm installation
        [System.Environment]::SetEnvironmentVariable('PNPM_HOME', $null, [System.EnvironmentVariableTarget]::Machine)
        [System.Environment]::SetEnvironmentVariable('PNPM_HOME', $null, [System.EnvironmentVariableTarget]::User)

        # installs pnpm
        npm install -g "pnpm@latest-$($enginesPnpmVersion.Major)"
        if ($LASTEXITCODE -ne 0) {
            Exit-WithError 'Failed to install pnpm'
        }

        # adds NPM global modules to the PATH
        if (Test-Path "$env:APPDATA\npm" -PathType Container) {
            Add-DirectoryToPath "$env:APPDATA\npm"
        }
    } elseif ($currentPnpmVersion -and $enginesPnpmVersion -and $currentPnpmVersion.CompareTo($enginesPnpmVersion) -lt 0) {
        Exit-WithError "Current pnpm version: $currentPnpmVersion (required: $enginesPnpmVersion)" `
            'Uninstall the current version of pnpm and run this script again'
    }
}

if ($LASTEXITCODE -ne 0) {
    Exit-WithError "Something went wrong, exit code: $LASTEXITCODE"
}

if (-not $env:CI) {
    Write-Host
    Write-Host 'Your machine has been setup for OneLauncher development!' -ForegroundColor Green
    Write-Host
    Read-Host 'Press Enter to continue'
}
