# Copyright © 2022-present Bencher
# Licensed under the Apache License, Version 2.0 (https://opensource.org/licenses/Apache-2.0/)
# or MIT License (https://opensource.org/licenses/MIT/) at your discretion.

<#
.SYNOPSIS

Bencher CLI v0.5.6 Installer

.DESCRIPTION

This script detects what platform you're on and fetches an appropriate archive from https://bencher.dev.
It then unpacks the binaries and installs them to $env:CARGO_HOME\bin ($HOME\.cargo\bin).
If $env:CARGO_HOME\bin does not exist, it falls back to creating it.
It will then add that dir to PATH by editing your Environment.Path registry key.

If you get an error that says "running scripts is disabled on this system":
- `Open Powershell` with `Run as Administrator`
- Run: `Set-ExecutionPolicy -ExecutionPolicy RemoteSigned`
- Enter: `Y`
- Rerun this script

.PARAMETER AppVersion
The version of the application to install

.PARAMETER ArtifactDownloadUrl
The URL of the directory where artifacts can be fetched from

.PARAMETER NoModifyPath
Don't add the install directory to PATH

.PARAMETER Help
Print help

#>

param (
  [Parameter(HelpMessage = "The version of the application to install")]
  [string]$AppVersion = $(if ($env:BENCHER_VERSION) { $env:BENCHER_VERSION } else { '0.5.6' }),
  [Parameter(HelpMessage = "The URL of the directory where artifacts can be fetched from")]
  [string]$ArtifactDownloadUrl = "https://bencher.dev/download/$(if ($env:BENCHER_VERSION) { $env:BENCHER_VERSION } else { '0.5.6' })",
  [Parameter(HelpMessage = "Don't add the install directory to PATH")]
  [switch]$NoModifyPath,
  [Parameter(HelpMessage = "Print Help")]
  [switch]$Help
)

$app_name = 'bencher'

function Install-Binary($install_args) {
  if ($Help) {
    Get-Help $PSCommandPath -Detailed
    Exit
  }

  Initialize-Environment

  $platforms = @{
    "x86_64-pc-windows-msvc" = @{
      "artifact_name" = "$app_name-v$AppVersion-windows-x86-64.exe"
      "zip_ext" = ""
      "bins" = "bencher"
      "bin" = "bencher"
    }
    "aarch64-pc-windows-msvc" = @{
      "artifact_name" = "$app_name-v$AppVersion-windows-arm-64.exe"
      "zip_ext" = ""
      "bins" = "bencher"
      "bin" = "bencher"
    }
  }

  $fetched = Download "$ArtifactDownloadUrl" $platforms
  # FIXME: add a flag that lets the user not do this step
  Invoke-Installer $fetched "$install_args"
}

function Get-TargetTriple() {
  try {
    # NOTE: this might return X64 on ARM64 Windows, which is OK since emulation is available.
    # It works correctly starting in PowerShell Core 7.3 and Windows PowerShell in Win 11 22H2.
    # Ideally this would just be
    #   [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    # but that gets a type from the wrong assembly on Windows PowerShell (i.e. not Core)
    $a = [System.Reflection.Assembly]::LoadWithPartialName("System.Runtime.InteropServices.RuntimeInformation")
    $t = $a.GetType("System.Runtime.InteropServices.RuntimeInformation")
    $p = $t.GetProperty("OSArchitecture")
    # Possible OSArchitecture Values: https://learn.microsoft.com/dotnet/api/system.runtime.interopservices.architecture
    # Rust supported platforms: https://doc.rust-lang.org/stable/rustc/platform-support.html
    switch ($p.GetValue($null).ToString())
    {
      "X86" { return "i686-pc-windows-msvc" }
      "X64" { return "x86_64-pc-windows-msvc" }
      "Arm" { return "thumbv7a-pc-windows-msvc" }
      "Arm64" { return "aarch64-pc-windows-msvc" }
    }
  } catch {
    # The above was added in .NET 4.7.1, so Windows PowerShell in versions of Windows
    # prior to Windows 10 v1709 may not have this API.
    Write-Verbose "Failed to get OS architecture for target triple"
    Write-Verbose $_
  }

  # This is available in .NET 4.0. We already checked for PS 5, which requires .NET 4.5.
  Write-Verbose("Target triple falling back to Is64BitOperatingSystem.")
  if ([System.Environment]::Is64BitOperatingSystem) {
    return "x86_64-pc-windows-msvc"
  } else {
    return "i686-pc-windows-msvc"
  }
}

function Download($download_url, $platforms) {
  $arch = Get-TargetTriple

  if (-not $platforms.ContainsKey($arch)) {
    # X64 is well-supported, including in emulation on ARM64
    Write-Verbose "Architecture $arch is not available, falling back to X64"
    $arch = "x86_64-pc-windows-msvc"
  }

  if (-not $platforms.ContainsKey($arch)) {
    # should not be possible, as currently we always produce X64 binaries.
    $platforms_json = ConvertTo-Json $platforms
    throw @"
Error: There isn't a package for $arch
If you would like for there to be, please open an issue on GitHub:
https://github.com/bencherdev/bencher/issues
"@
  }

  # Lookup what we expect this platform to look like
  $info = $platforms[$arch]
  $artifact_name = $info["artifact_name"]
  $zip_ext = $info["zip_ext"]
  $bin_names = $info["bins"]
  $bin_name = $info["bin"]

  # Make a new temp dir to unpack things to
  $tmp = New-Temp-Dir
  $dir_path = "$tmp\input$zip_ext"

  # Download and unpack!
  $url = "$download_url/$artifact_name"
  Write-Information "Downloading Bencher CLI ($app_name $AppVersion)"
  Write-Verbose "  From: $url"
  Write-Verbose "  To:   $dir_path"
  $wc = New-Object Net.Webclient
  $wc.downloadFile($url, $dir_path)

  # Select the tool to unpack the files with.
  #
  # As of windows 10(?), powershell comes with tar preinstalled, but in practice
  # it only seems to support .tar.gz, and not xz/zstd. Still, we should try to
  # forward all tars to it in case the user has a machine that can handle it!
  switch -Wildcard ($zip_ext) {
    ".zip" {
      Expand-Archive -Path $dir_path -DestinationPath "$tmp";
      Break
    }
    ".tar.*" {
      tar xf $dir_path --strip-components 1 -C "$tmp";
      Break
    }
    "" {
      Write-Verbose "Installing single binary: $bin_name.exe"
      Copy-Item -Path "$dir_path" -Destination "$tmp\$bin_name.exe"
      $bin_names = "$bin_name.exe"
      Break
    }
    Default {
      throw "ERROR: Unknown archive format: $zip_ext"
    }
  }

  # Let the next step know what to copy
  $bin_paths = @()
  foreach ($bin_name in $bin_names) {
    Write-Verbose "Unpacked: $bin_name"
    $bin_paths += "$tmp\$bin_name"
  }
  return $bin_paths
}

function Invoke-Installer($bin_paths) {
  # first try CARGO_HOME, then fallback to HOME
  $dest_dir = if (($base_dir = $env:CARGO_HOME)) {
    Join-Path $base_dir "bin"
  } elseif (($base_dir = $HOME)) {
    Join-Path $base_dir ".cargo\bin"
  } else {
    throw "ERROR: Home not found: Could not find your CARGO_HOME or HOME dir to install binaries"
  }
  $dest_dir = New-Item -Force -ItemType Directory -Path $dest_dir
  Write-Information "Installing Bencher CLI to $dest_dir"
  # Just copy the binaries from the temp location to the install dir
  foreach ($bin_path in $bin_paths) {
    $installed_file = Split-Path -Path "$bin_path" -Leaf
    Copy-Item "$bin_path" -Destination "$dest_dir"
    Remove-Item "$bin_path" -Recurse -Force
    Write-Verbose "Installed: $installed_file"
  }

  $rabbit16 = [System.Convert]::toInt32("1F430", 16)
  $rabbit32 = [System.Char]::ConvertFromUtf32($rabbit16)
  Write-Information "$rabbit32 Bencher CLI installed!"
  if (-not $NoModifyPath) {
    if (Add-Path $dest_dir) {
        Write-Information ""
        Write-Information "$dest_dir was added to your PATH, you may need to restart your shell for that to take effect."
    }
  }
}

# Try to add the given path to PATH via the registry
#
# Returns true if the registry was modified, otherwise returns false
# (indicating it was already on PATH)
function Add-Path($OrigPathToAdd) {
  $RegistryPath = "HKCU:\Environment"
  $PropertyName = "Path"
  $PathToAdd = $OrigPathToAdd

  $Item = if (Test-Path $RegistryPath) {
    # If the registry key exists, get it
    Get-Item -Path $RegistryPath
  } else {
    # If the registry key doesn't exist, create it
    Write-Verbose  "Creating $RegistryPath"
    New-Item -Path $RegistryPath -Force
  }

  $OldPath = ""
  try {
    # Try to get the old PATH value. If that fails, assume we're making it from scratch.
    # Otherwise assume there's already paths in here and use a ; separator
    $OldPath = $Item | Get-ItemPropertyValue -Name $PropertyName
    $PathToAdd = "$PathToAdd;"
  } catch {
    # We'll be creating the PATH from scratch
    Write-Verbose "Adding $PropertyName property to $RegistryPath"
  }

  # Check if the path is already there
  #
  # We don't want to incorrectly match "C:\blah\" to "C:\blah\blah\", so we include the semicolon
  # delimiters when searching, ensuring exact matches. To avoid corner cases we add semicolons to
  # both sides of the input, allowing us to pretend we're always in the middle of a list.
  if (";$OldPath;" -like "*;$OrigPathToAdd;*") {
    # Already on path, nothing to do
    Write-Verbose "$OrigPathToAdd already in PATH"
    return $false
  } else {
    # Actually update PATH
    Write-Verbose "Adding $OrigPathToAdd to PATH"
    $NewPath = $PathToAdd + $OldPath
    # We use -Force here to make the value already existing not be an error
    $Item | New-ItemProperty -Name $PropertyName -Value $NewPath -PropertyType String -Force | Out-Null
    return $true
  }
}

function Initialize-Environment() {
  If (($PSVersionTable.PSVersion.Major) -lt 5) {
    throw @"
Error: PowerShell 5 or later is required to install $app_name.
Upgrade PowerShell:

    https://docs.microsoft.com/en-us/powershell/scripting/setup/installing-windows-powershell

"@
  }

  # show notification to change execution policy:
  $allowedExecutionPolicy = @('Unrestricted', 'RemoteSigned', 'ByPass')
  If ((Get-ExecutionPolicy).ToString() -notin $allowedExecutionPolicy) {
    throw @"
Error: PowerShell requires an execution policy in [$($allowedExecutionPolicy -join ", ")] to run $app_name. For example, to set the execution policy to 'RemoteSigned' please run:

    Set-ExecutionPolicy RemoteSigned -scope CurrentUser

"@
  }

  # GitHub requires TLS 1.2
  If ([System.Enum]::GetNames([System.Net.SecurityProtocolType]) -notcontains 'Tls12') {
    throw @"
Error: Installing $app_name requires at least .NET Framework 4.5
Please download and install it first:

    https://www.microsoft.com/net/download

"@
  }
}

function New-Temp-Dir() {
  [CmdletBinding(SupportsShouldProcess)]
  param()
  $parent = [System.IO.Path]::GetTempPath()
  [string] $name = [System.Guid]::NewGuid()
  New-Item -ItemType Directory -Path (Join-Path $parent $name)
}

# PSScriptAnalyzer doesn't like how we use our params as globals, this calms it
$Null = $ArtifactDownloadUrl, $NoModifyPath, $Help
# Make Write-Information statements be visible
$InformationPreference = "Continue"

# The default interactive handler
try {
  Install-Binary "$Args"
} catch {
  Write-Information $_
  exit 1
}
