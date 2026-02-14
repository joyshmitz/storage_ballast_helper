#Requires -Version 5.1
<#
.SYNOPSIS
    sbh Windows installer with deterministic verification and rollback support.

.DESCRIPTION
    Downloads and installs the sbh (Storage Ballast Helper) binary for Windows.
    Supports version pinning, SHA-256 checksum verification, PATH automation,
    dry-run mode, JSON output, and structured event logging.

.PARAMETER Version
    Release tag or semver to install (default: latest).

.PARAMETER Dest
    Destination directory for the sbh binary.

.PARAMETER User
    Install to user location (default: $env:LOCALAPPDATA\sbh\bin).

.PARAMETER System
    Install to system location (C:\Program Files\sbh\bin). Requires elevation.

.PARAMETER DryRun
    Print planned actions without changing the system.

.PARAMETER Verify
    Enforce SHA-256 checksum verification (default).

.PARAMETER NoVerify
    Skip checksum verification (unsafe, logged).

.PARAMETER Json
    Emit machine-readable JSON summary to stdout.

.PARAMETER TraceId
    Set explicit trace id for event correlation.

.PARAMETER EventLog
    Append per-phase JSONL events to the given file path.

.PARAMETER Quiet
    Reduce output to errors only.

.PARAMETER NoColor
    Disable ANSI colors in terminal output.

.PARAMETER AddToPath
    Add destination directory to user PATH (idempotent).

.PARAMETER NoPath
    Skip PATH modification even if destination is not in PATH.

.PARAMETER Help
    Show this help text and exit.

.EXAMPLE
    irm https://raw.githubusercontent.com/Dicklesworthstone/storage_ballast_helper/main/scripts/install.ps1 | iex
    # Install latest release to user directory

.EXAMPLE
    .\scripts\install.ps1 -Version v0.1.0 -System -DryRun
    # Dry-run system install of specific version

.EXAMPLE
    .\scripts\install.ps1 -Json -AddToPath
    # Install with JSON output and PATH automation
#>
[CmdletBinding(DefaultParameterSetName = 'User')]
param(
    [string]$Version = 'latest',
    [string]$Dest = '',
    [Parameter(ParameterSetName = 'User')]
    [switch]$User,
    [Parameter(ParameterSetName = 'System')]
    [switch]$System,
    [switch]$DryRun,
    [switch]$Verify,
    [switch]$NoVerify,
    [switch]$Json,
    [string]$TraceId = '',
    [string]$EventLog = '',
    [switch]$Quiet,
    [switch]$NoColor,
    [switch]$AddToPath,
    [switch]$NoPath,
    [Alias('h')]
    [switch]$Help
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

$Script:Program       = 'sbh'
$Script:RepoDefault   = 'Dicklesworthstone/storage_ballast_helper'
$Script:Repo          = if ($env:SBH_REPOSITORY) { $env:SBH_REPOSITORY } else { $Script:RepoDefault }
$Script:DestMode      = if ($System) { 'system' } else { 'user' }
$Script:DoVerify      = -not $NoVerify
$Script:CurrentPhase  = 'init'
$Script:PhaseStart    = Get-Date
$Script:WorkDir       = ''
$Script:AssetUrl      = ''
$Script:ChecksumUrl   = ''
$Script:TargetTriple  = ''
$Script:ReleaseTag    = ''
$Script:InstallChanged = $false
$Script:BackupPath    = ''

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

function Write-Header {
    param([string]$Message)
    if ($Quiet) { return }
    if (-not $NoColor -and $Host.UI.SupportsVirtualTerminal) {
        Write-Host "`e[1;34m==> $Message`e[0m"
    } else {
        Write-Host "==> $Message"
    }
}

function Write-Info {
    param([string]$Message)
    if ($Quiet) { return }
    Write-Host $Message
}

function Write-Warn {
    param([string]$Message)
    if (-not $NoColor -and $Host.UI.SupportsVirtualTerminal) {
        Write-Host "`e[1;33mWARN: $Message`e[0m" -ForegroundColor Yellow
    } else {
        Write-Warning $Message
    }
}

function Write-Err {
    param([string]$Message)
    if (-not $NoColor -and $Host.UI.SupportsVirtualTerminal) {
        Write-Host "`e[1;31mERROR: $Message`e[0m" -ForegroundColor Red
    } else {
        Write-Error $Message -ErrorAction Continue
    }
}

function Get-VerifyModeLabel {
    if ($Script:DoVerify) { 'enforced' } else { 'bypassed' }
}

function Get-JsonBool {
    param([bool]$Value)
    if ($Value) { 'true' } else { 'false' }
}

function ConvertTo-JsonSafe {
    param([string]$Value)
    $Value -replace '\\', '\\' -replace '"', '\"'
}

function Initialize-Trace {
    if (-not $Script:TraceId) {
        $timestamp = (Get-Date -Format 'yyyyMMddTHHmmssZ')
        $pid_val   = $PID
        $rand      = Get-Random -Maximum 99999
        $Script:TraceId = "install-$timestamp-$pid_val-$rand"
    }
}

function Emit-EventJson {
    param(
        [string]$Phase,
        [string]$Status,
        [string]$Message,
        [int]$DurationSeconds
    )
    $ts = (Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ')
    @"
{"ts":"$ts","trace_id":"$(ConvertTo-JsonSafe $Script:TraceId)","phase":"$Phase","status":"$Status","message":"$(ConvertTo-JsonSafe $Message)","mode":"$Script:DestMode","version":"$(ConvertTo-JsonSafe $Version)","target":"$Script:TargetTriple","destination":"$(ConvertTo-JsonSafe $Dest)","verify":"$(Get-VerifyModeLabel)","dry_run":$(Get-JsonBool $DryRun),"duration_seconds":$DurationSeconds}
"@
}

function Emit-Event {
    param(
        [string]$Phase,
        [string]$Status,
        [string]$Message,
        [int]$DurationSeconds
    )
    Initialize-Trace
    $payload = Emit-EventJson -Phase $Phase -Status $Status -Message $Message -DurationSeconds $DurationSeconds

    if ($EventLog) {
        $logDir = Split-Path -Parent $EventLog
        if ($logDir -and -not (Test-Path $logDir)) {
            New-Item -ItemType Directory -Path $logDir -Force | Out-Null
        }
        $payload | Out-File -FilePath $EventLog -Append -Encoding utf8
    }

    if (-not $Json -and -not $Quiet) {
        Write-Host "[trace:$($Script:TraceId)] $Phase/${Status}: $Message"
    }
}

function Start-Phase {
    param([string]$Name, [string]$Message)
    $Script:CurrentPhase = $Name
    $Script:PhaseStart = Get-Date
    Emit-Event -Phase $Name -Status 'start' -Message $Message -DurationSeconds 0
}

function Complete-Phase {
    param([string]$Message)
    $duration = [int]((Get-Date) - $Script:PhaseStart).TotalSeconds
    Emit-Event -Phase $Script:CurrentPhase -Status 'success' -Message $Message -DurationSeconds $duration
}

function Stop-WithError {
    param([string]$Message)
    $duration = [int]((Get-Date) - $Script:PhaseStart).TotalSeconds
    Emit-Event -Phase $Script:CurrentPhase -Status 'failure' -Message $Message -DurationSeconds $duration

    # Attempt rollback if we have a backup.
    if ($Script:BackupPath -and (Test-Path $Script:BackupPath)) {
        $target = Join-Path $Dest "$Script:Program.exe"
        try {
            Copy-Item -Path $Script:BackupPath -Destination $target -Force
            Emit-Event -Phase 'rollback' -Status 'success' -Message "Rolled back to previous binary" -DurationSeconds 0
        } catch {
            Emit-Event -Phase 'rollback' -Status 'failure' -Message "Rollback failed: $_" -DurationSeconds 0
        }
    }

    if ($Json) {
        Emit-JsonResult -Status 'error' -Message $Message -Changed $false
    } else {
        Write-Err $Message
    }
    exit 1
}

function Emit-JsonResult {
    param(
        [string]$Status,
        [string]$Message,
        [bool]$Changed
    )
    $output = @"
{"program":"$Script:Program","status":"$Status","message":"$(ConvertTo-JsonSafe $Message)","mode":"$Script:DestMode","destination":"$(ConvertTo-JsonSafe $Dest)","target":"$Script:TargetTriple","version":"$(ConvertTo-JsonSafe $Version)","asset_url":"$(ConvertTo-JsonSafe $Script:AssetUrl)","trace_id":"$(ConvertTo-JsonSafe $Script:TraceId)","verify":"$(Get-VerifyModeLabel)","changed":$(Get-JsonBool $Changed),"dry_run":$(Get-JsonBool $DryRun)}
"@
    Write-Output $output
}

# ---------------------------------------------------------------------------
# Platform detection
# ---------------------------------------------------------------------------

function Resolve-TargetTriple {
    $arch = $env:PROCESSOR_ARCHITECTURE
    switch ($arch) {
        'AMD64'   { $Script:TargetTriple = 'x86_64-pc-windows-msvc' }
        'ARM64'   { $Script:TargetTriple = 'aarch64-pc-windows-msvc' }
        default   { Stop-WithError "Unsupported Windows architecture: $arch" }
    }
}

function Resolve-Destination {
    if ($Dest) { return }

    if ($System) {
        $Dest = Join-Path $env:ProgramFiles 'sbh\bin'
    } else {
        $Dest = Join-Path $env:LOCALAPPDATA 'sbh\bin'
    }
    # Update the script-level variable.
    Set-Variable -Name 'Dest' -Value $Dest -Scope 1
}

function Normalize-Tag {
    if ($Version -eq 'latest') {
        $Script:ReleaseTag = 'latest'
    } elseif ($Version.StartsWith('v')) {
        $Script:ReleaseTag = $Version
    } else {
        $Script:ReleaseTag = "v$Version"
    }
}

function Build-Urls {
    $asset    = "$Script:Program-$Script:TargetTriple.tar.xz"
    $checksum = "$asset.sha256"

    if ($env:SBH_INSTALLER_ASSET_URL) {
        $Script:AssetUrl = $env:SBH_INSTALLER_ASSET_URL
    } elseif ($Script:ReleaseTag -eq 'latest') {
        $Script:AssetUrl = "https://github.com/$Script:Repo/releases/latest/download/$asset"
    } else {
        $Script:AssetUrl = "https://github.com/$Script:Repo/releases/download/$Script:ReleaseTag/$asset"
    }

    if ($env:SBH_INSTALLER_CHECKSUM_URL) {
        $Script:ChecksumUrl = $env:SBH_INSTALLER_CHECKSUM_URL
    } elseif ($Script:ReleaseTag -eq 'latest') {
        $Script:ChecksumUrl = "https://github.com/$Script:Repo/releases/latest/download/$checksum"
    } else {
        $Script:ChecksumUrl = "https://github.com/$Script:Repo/releases/download/$Script:ReleaseTag/$checksum"
    }
}

# ---------------------------------------------------------------------------
# Checksum verification
# ---------------------------------------------------------------------------

function Get-FileSha256 {
    param([string]$FilePath)
    $hash = Get-FileHash -Path $FilePath -Algorithm SHA256
    return $hash.Hash.ToLower()
}

# ---------------------------------------------------------------------------
# PATH management
# ---------------------------------------------------------------------------

function Test-InPath {
    param([string]$Dir)
    $current = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not $current) { return $false }
    $entries = $current -split ';' | ForEach-Object { $_.TrimEnd('\') }
    return ($entries -contains $Dir.TrimEnd('\'))
}

function Add-ToUserPath {
    param([string]$Dir)
    if (Test-InPath -Dir $Dir) {
        Write-Info "PATH: $Dir is already in user PATH"
        return
    }

    $current = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($current) {
        $newPath = "$current;$Dir"
    } else {
        $newPath = $Dir
    }
    [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    Write-Info "PATH: Added $Dir to user PATH (effective in new shells)"
}

# ---------------------------------------------------------------------------
# Installation
# ---------------------------------------------------------------------------

function Install-Binary {
    param(
        [string]$SourcePath,
        [string]$TargetPath
    )

    $targetDir = Split-Path -Parent $TargetPath

    # Create destination directory.
    if (-not (Test-Path $targetDir)) {
        try {
            New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
        } catch {
            Stop-WithError "Cannot create destination directory: $targetDir. $_"
        }
    }

    # Check if binary is identical (idempotent install).
    if (Test-Path $TargetPath) {
        $srcHash = Get-FileSha256 -FilePath $SourcePath
        $dstHash = Get-FileSha256 -FilePath $TargetPath
        if ($srcHash -eq $dstHash) {
            $Script:InstallChanged = $false
            return
        }

        # Back up existing binary for rollback.
        $Script:BackupPath = "$TargetPath.bak"
        try {
            Copy-Item -Path $TargetPath -Destination $Script:BackupPath -Force
        } catch {
            Write-Warn "Could not back up existing binary: $_"
        }
    }

    try {
        Copy-Item -Path $SourcePath -Destination $TargetPath -Force
        $Script:InstallChanged = $true
    } catch {
        Stop-WithError "Cannot write to $TargetPath. $_"
    }
}

function Print-Summary {
    param(
        [string]$Message,
        [bool]$Changed
    )

    if ($Json) {
        Emit-JsonResult -Status 'ok' -Message $Message -Changed $Changed
        return
    }

    Write-Header "sbh installer summary"
    Write-Info "Mode:        $Script:DestMode"
    Write-Info "Version:     $Version"
    Write-Info "Target:      $Script:TargetTriple"
    Write-Info "Destination: $(Join-Path $Dest "$Script:Program.exe")"
    Write-Info "Trace ID:    $Script:TraceId"
    Write-Info "Verify:      $(Get-VerifyModeLabel)"
    Write-Info "Asset:       $Script:AssetUrl"
    if ($EventLog) {
        Write-Info "Event log:   $EventLog"
    }
    Write-Info "Result:      $Message"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

function Main {
    if ($Help) {
        Get-Help $MyInvocation.ScriptName -Detailed
        return
    }

    Initialize-Trace
    Start-Phase -Name 'prepare' -Message 'resolving prerequisites and installer contract'

    Resolve-TargetTriple
    Resolve-Destination
    Normalize-Tag
    Build-Urls
    Complete-Phase -Message 'resolved prerequisites and artifact contract'

    if (-not $Script:DoVerify) {
        Write-Warn 'Checksum verification is disabled (-NoVerify).'
    }

    # Dry-run mode.
    if ($DryRun) {
        Start-Phase -Name 'dry_run' -Message 'rendering dry-run execution plan'
        if (-not $Json) {
            Write-Header "sbh installer (dry-run)"
            Write-Info "Would download: $Script:AssetUrl"
            if ($Script:DoVerify) {
                Write-Info "Would download checksum: $Script:ChecksumUrl"
            }
            Write-Info "Would install to: $(Join-Path $Dest "$Script:Program.exe")"
            if ($AddToPath -and -not $NoPath) {
                Write-Info "Would add to PATH: $Dest"
            }
        }
        Complete-Phase -Message 'dry-run plan generated'
        Print-Summary -Message 'dry-run complete (no changes applied)' -Changed $false
        Emit-Event -Phase 'complete' -Status 'success' -Message 'installer finished in dry-run mode' -DurationSeconds 0
        return
    }

    # Create temporary work directory.
    $Script:WorkDir = Join-Path ([System.IO.Path]::GetTempPath()) "sbh-install-$(Get-Random)"
    New-Item -ItemType Directory -Path $Script:WorkDir -Force | Out-Null

    try {
        $archivePath  = Join-Path $Script:WorkDir 'artifact.tar.xz'
        $checksumPath = Join-Path $Script:WorkDir 'artifact.sha256'
        $extractDir   = Join-Path $Script:WorkDir 'extract'
        $targetPath   = Join-Path $Dest "$Script:Program.exe"

        # Download release artifact.
        Start-Phase -Name 'download_artifact' -Message 'downloading release artifact'
        Write-Header 'Downloading release artifact'
        try {
            $ProgressPreference = 'SilentlyContinue'
            Invoke-WebRequest -Uri $Script:AssetUrl -OutFile $archivePath -UseBasicParsing
        } catch {
            Stop-WithError "Failed to download release artifact from $($Script:AssetUrl): $_"
        }
        Complete-Phase -Message 'release artifact downloaded'

        # Verify checksum.
        if ($Script:DoVerify) {
            Start-Phase -Name 'verify_artifact' -Message 'verifying artifact checksum'
            Write-Header 'Verifying checksum'
            try {
                Invoke-WebRequest -Uri $Script:ChecksumUrl -OutFile $checksumPath -UseBasicParsing
            } catch {
                Stop-WithError "Failed to download checksum from $($Script:ChecksumUrl): $_"
            }

            $expectedRaw = (Get-Content -Path $checksumPath -Raw).Trim()
            $expected    = ($expectedRaw -split '\s+')[0].ToLower()
            if (-not $expected) {
                Stop-WithError 'Checksum file is empty or malformed'
            }

            $actual = Get-FileSha256 -FilePath $archivePath
            if ($expected -ne $actual) {
                Stop-WithError "Checksum mismatch. Expected $expected, got $actual."
            }
            Complete-Phase -Message 'artifact checksum verified'
        }

        # Extract archive.
        Start-Phase -Name 'extract_artifact' -Message 'extracting release archive'
        Write-Header 'Extracting archive'
        New-Item -ItemType Directory -Path $extractDir -Force | Out-Null

        # Try tar (available on Windows 10+), fall back to 7-Zip.
        $tarExe = Get-Command 'tar' -ErrorAction SilentlyContinue
        if ($tarExe) {
            & tar -xJf $archivePath -C $extractDir 2>&1 | Out-Null
            if ($LASTEXITCODE -ne 0) {
                Stop-WithError 'Failed to extract archive with tar'
            }
        } elseif (Get-Command '7z' -ErrorAction SilentlyContinue) {
            & 7z x $archivePath -o"$extractDir" -y 2>&1 | Out-Null
            # 7z may produce a .tar, extract that too.
            $innerTar = Get-ChildItem -Path $extractDir -Filter '*.tar' -File | Select-Object -First 1
            if ($innerTar) {
                & 7z x $innerTar.FullName -o"$extractDir" -y 2>&1 | Out-Null
            }
        } else {
            Stop-WithError 'Neither tar nor 7z found. Install Windows tar (included in Windows 10 1803+) or 7-Zip.'
        }

        # Find the binary in the extracted files.
        $binaryPath = Get-ChildItem -Path $extractDir -Recurse -Filter "$Script:Program.exe" -File |
                      Select-Object -First 1 -ExpandProperty FullName
        if (-not $binaryPath) {
            # Also try without .exe extension (binary may not have it in the archive).
            $binaryPath = Get-ChildItem -Path $extractDir -Recurse -Filter $Script:Program -File |
                          Select-Object -First 1 -ExpandProperty FullName
        }
        if (-not $binaryPath) {
            Stop-WithError "Downloaded archive does not contain '$Script:Program' binary"
        }
        Complete-Phase -Message 'release archive extracted'

        # Install binary.
        Start-Phase -Name 'install_binary' -Message 'installing sbh binary'
        Write-Header 'Installing binary'
        Install-Binary -SourcePath $binaryPath -TargetPath $targetPath
        Complete-Phase -Message 'binary install phase completed'

        # PATH automation.
        if ($AddToPath -and -not $NoPath -and -not $DryRun) {
            Start-Phase -Name 'path_setup' -Message 'configuring user PATH'
            Add-ToUserPath -Dir $Dest
            Complete-Phase -Message 'PATH configuration complete'
        } elseif (-not $NoPath -and -not (Test-InPath -Dir $Dest)) {
            Write-Warn "$Dest is not in PATH. Run with -AddToPath or add it manually."
        }

        # Clean up backup on success.
        if ($Script:BackupPath -and (Test-Path $Script:BackupPath)) {
            Remove-Item -Path $Script:BackupPath -Force -ErrorAction SilentlyContinue
        }

        if ($Script:InstallChanged) {
            Print-Summary -Message "installed $Script:Program to $targetPath" -Changed $true
            Emit-Event -Phase 'complete' -Status 'success' -Message 'installer completed with binary update' -DurationSeconds 0
        } else {
            Print-Summary -Message "$Script:Program already up to date at $targetPath" -Changed $false
            Emit-Event -Phase 'complete' -Status 'success' -Message 'installer completed without changes' -DurationSeconds 0
        }

    } finally {
        # Cleanup temporary files.
        if ($Script:WorkDir -and (Test-Path $Script:WorkDir)) {
            Remove-Item -Path $Script:WorkDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

Main
