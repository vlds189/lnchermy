# mc_console.ps1 - Unified Minecraft launcher & installer (vanilla + Forge)
$workDir = $PSScriptRoot
if (-not $workDir) { $workDir = Get-Location }

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$ProgressPreference = 'SilentlyContinue'

# =========================================================
#  Self-update configuration
# =========================================================
# Bump APP_VERSION on every release. The launcher fetches version.json from
# the repo and compares; if newer, it downloads mc_console.ps1 over itself.
$APP_VERSION    = "1.2.0"
$REPO_OWNER     = "vlds189"
$REPO_NAME      = "lnchermy"
$REPO_BRANCH    = "main"
# Raw URLs (GitHub serves these directly, no API/auth needed).
$UPDATE_VERSION_URL = "https://raw.githubusercontent.com/$REPO_OWNER/$REPO_NAME/$REPO_BRANCH/version.json"
$UPDATE_SCRIPT_URL  = "https://raw.githubusercontent.com/$REPO_OWNER/$REPO_NAME/$REPO_BRANCH/mc_console.ps1"

# Compares dotted version strings (returns -1/0/1, like a normal comparator).
function Compare-Version($a, $b) {
    $aa = "$a".Split('.'); $bb = "$b".Split('.')
    $max = [Math]::Max($aa.Count, $bb.Count)
    for ($i = 0; $i -lt $max; $i++) {
        $av = if ($i -lt $aa.Count) { [int]$aa[$i] } else { 0 }
        $bv = if ($i -lt $bb.Count) { [int]$bb[$i] } else { 0 }
        if ($av -ne $bv) { return ([Math]::Sign($av - $bv)) }
    }
    return 0
}

# Fetches latest version info. Returns $null on network failure.
function Get-LatestVersion {
    try {
        $info = Invoke-RestMethod $UPDATE_VERSION_URL -UseBasicParsing
        return $info.version
    } catch { return $null }
}

# Downloads the newest mc_console.ps1 and replaces the running script.
# Returns $true on success. The caller is expected to tell the user to restart.
function Invoke-SelfUpdate {
    $scriptPath = Join-Path $workDir "mc_console.ps1"
    $tmpPath    = Join-Path $workDir "mc_console.ps1.new"
    $bakPath    = Join-Path $workDir "mc_console.ps1.bak"

    Write-Host "Downloading update..." -ForegroundColor Cyan
    try {
        Invoke-WebRequest $UPDATE_SCRIPT_URL -OutFile $tmpPath -UseBasicParsing
    } catch {
        Write-Host "Download failed: $($_.Exception.Message)" -ForegroundColor Red
        return $false
    }
    if (-not (Test-Path $tmpPath) -or (Get-Item $tmpPath).Length -lt 1000) {
        Write-Host "Downloaded file looks empty/invalid." -ForegroundColor Red
        Remove-Item $tmpPath -Force -ErrorAction SilentlyContinue
        return $false
    }

    # Backup current, then swap in the new file.
    if (Test-Path $bakPath) { Remove-Item $bakPath -Force }
    if (Test-Path $scriptPath) { Move-Item $scriptPath $bakPath -Force }
    Move-Item $tmpPath $scriptPath -Force
    return $true
}

# Runs at startup. Silently checks for an update and asks the user if found.
function Check-UpdateOnStartup {
    $latest = Get-LatestVersion
    if (-not $latest) { return }   # network issue -> stay quiet
    if ((Compare-Version $latest $APP_VERSION) -le 0) { return }  # up to date

    Write-Host ""
    Write-Host "A new version is available: v$APP_VERSION -> v$latest" -ForegroundColor Yellow
    $ans = Read-Host "Update now? (y/n)"
    if ($ans -eq 'y' -or $ans -eq 'Y') {
        if (Invoke-SelfUpdate) {
            Write-Host "Update installed successfully." -ForegroundColor Green
            Write-Host "Please restart the launcher to use the new version." -ForegroundColor Cyan
            Read-Host "Press Enter to exit"
            exit 0
        } else {
            Write-Host "Update failed. You can continue with the current version." -ForegroundColor Yellow
            Start-Sleep -Seconds 2
        }
    }
}


# ---- Launch settings (defaults; overridden by mc_console_settings.json) ----
$UUID = "00000000-0000-0000-0000-000000000000"
$ACCESS_TOKEN = "0"
$USER_TYPE = "msa"

# Persistent settings stored in mc_console_settings.json next to the script.
$script:settingsFile = Join-Path $workDir "mc_console_settings.json"
$script:RAM_MIN = "2G"
$script:RAM_MAX = "4G"
$script:contentIndexUrl = ""   # URL of the content index JSON (mods/resourcepacks/shaders)
$script:USERNAME = "Player"

function Load-Settings {
    if (Test-Path $script:settingsFile) {
        try {
            $s = Get-Content $script:settingsFile -Raw | ConvertFrom-Json
            if ($s.RAM_MIN) { $script:RAM_MIN = $s.RAM_MIN }
            if ($s.RAM_MAX) { $script:RAM_MAX = $s.RAM_MAX }
            if ($s.ContentIndexUrl) { $script:contentIndexUrl = $s.ContentIndexUrl }
            if ($s.Username) { $script:USERNAME = $s.Username }
        } catch { }
    }
}

function Save-Settings {
    $obj = @{ RAM_MIN = $script:RAM_MIN; RAM_MAX = $script:RAM_MAX; ContentIndexUrl = $script:contentIndexUrl; Username = $script:USERNAME } | ConvertTo-Json
    [System.IO.File]::WriteAllText($script:settingsFile, $obj)
}

Load-Settings
$VERSION_TYPE = "release"

# Check for updates before showing the menu (non-fatal on network failure).
Check-UpdateOnStartup

# =========================================================
#  Menus
# =========================================================
function Show-MainMenu {
    Clear-Host
    Write-Host "================================" -ForegroundColor Cyan
    Write-Host "       Minecraft Console" -ForegroundColor Cyan
    Write-Host "================================" -ForegroundColor Cyan
    Write-Host "1. Launch Minecraft   (scan installed versions)"
    Write-Host "2. Install Minecraft  (download vanilla version)"
    Write-Host "3. Install Forge      (download & open installer)"
    Write-Host "4. Install Java       (download portable Java)"
    Write-Host "5. Settings           (memory, content index)"
    Write-Host "6. Download content   (mods, resourcepacks, shaders)"
    Write-Host "7. Install OptiFine   (download & open installer)"
    Write-Host "0. Exit"
}

function Pause-Host {
    Write-Host ""
    Read-Host "Press Enter to continue"
}

# =========================================================
#  Helpers
# =========================================================
function Get-OsName {
    if ($PSVersionTable.PSEdition -eq 'Desktop') { return 'windows' }
    if ($IsWindows) { return 'windows' }
    if ($IsLinux)   { return 'linux' }
    if ($IsOsX)     { return 'osx' }
    return 'windows'
}

# Evaluate Mojang "rules" for the current OS and enabled features.
# $enabledFeatures is a hashtable of feature => $true for those we turn on.
# With no features enabled (normal offline launch), feature-gated args such as
# --demo, --width/--height and --quickPlay* are correctly excluded.
function Test-RulesAllowed($rules, $enabledFeatures) {
    if (-not $rules -or $rules.Count -eq 0) { return $true }
    $os = Get-OsName
    $allowed = $false
    foreach ($rule in $rules) {
        $applies = $true

        # OS condition
        if ($rule.os) {
            $applies = ($rule.os.name -eq $os)
            # optional os arch/version check
            if ($applies -and $rule.os.arch) {
                $is64 = [Environment]::Is64BitOperatingSystem
                if ($rule.os.arch -eq 'x86' -and $is64) { $applies = $false }
            }
        }

        # Features condition: rule applies only if ALL listed features match
        # our enabled set. (When features is present, it's effectively a filter.)
        if ($applies -and $rule.features) {
            foreach ($f in $rule.features.PSObject.Properties.Name) {
                $want = [bool]$rule.features.$f
                $have = [bool]$enabledFeatures.$f
                if ($want -ne $have) { $applies = $false; break }
            }
        }

        if ($applies) { $allowed = ($rule.action -eq 'allow') }
    }
    return $allowed
}

# group:artifact:version[:classifier] -> maven repository relative path
function Get-MavenRelPath($name, $classifier) {
    $parts = $name -split ':'
    if ($parts.Count -lt 3) { return $null }
    $group    = $parts[0] -replace '\.', '/'
    $artifact = $parts[1]
    $version  = $parts[2]
    $file = if ($classifier) { "$artifact-$version-$classifier.jar" } else { "$artifact-$version.jar" }
    return "$group/$artifact/$version/$file"
}

function Resolve-LibPath($lib, $libDir, $classifier) {
    # 1. Explicit downloads path (most reliable)
    if ($classifier) {
        if ($lib.downloads -and $lib.downloads.classifiers) {
            $cl = $lib.downloads.classifiers.$classifier
            if ($cl -and $cl.path) { return [System.IO.Path]::Combine($libDir, $cl.path) }
        }
    } else {
        if ($lib.downloads -and $lib.downloads.artifact -and $lib.downloads.artifact.path) {
            return [System.IO.Path]::Combine($libDir, $lib.downloads.artifact.path)
        }
    }
    # 2. Build from maven coordinates
    if ($lib.name) {
        $rel = Get-MavenRelPath $lib.name $classifier
        if ($rel) { return [System.IO.Path]::Combine($libDir, $rel) }
    }
    return $null
}

# Replace ${var} placeholders in a single argument string
function Resolve-Placeholders($text, $vars) {
    if ($text -isnot [string]) { return "$text" }
    $out = $text
    foreach ($key in $vars.Keys) {
        $out = $out.Replace('${' + $key + '}', $vars[$key])
    }
    return $out
}

# Expand an arguments list (game or jvm) applying OS/feature rules & placeholders.
function Expand-Arguments($list, $vars, $enabledFeatures) {
    $result = New-Object System.Collections.Generic.List[string]
    if (-not $list) { return $result }
    foreach ($item in $list) {
        if ($item -is [string]) {
            $result.Add((Resolve-Placeholders $item $vars)) | Out-Null
        } else {
            # object with optional rules + value
            if ($item.PSObject.Properties['rules']) {
                if (-not (Test-RulesAllowed $item.rules $enabledFeatures)) { continue }
            }
            $val = $item.value
            if ($val -is [array]) {
                foreach ($v in $val) { $result.Add((Resolve-Placeholders $v $vars)) | Out-Null }
            } else {
                $result.Add((Resolve-Placeholders $val $vars)) | Out-Null
            }
        }
    }
    return $result
}

# Load a version JSON, merging its parent (inheritsFrom) recursively.
function Get-ResolvedVersionJson($versionName, $versionsRoot) {
    $vDir  = Join-Path $versionsRoot $versionName
    $vJson = Join-Path $vDir "$versionName.json"
    if (-not (Test-Path $vJson)) { return $null }

    $json = Get-Content $vJson -Raw | ConvertFrom-Json

    # Merge parent first (vanilla base for Forge)
    if ($json.inheritsFrom) {
        $parent = Get-ResolvedVersionJson $json.inheritsFrom $versionsRoot
        if ($parent) {
            # libraries: parent + child
            $libs = New-Object System.Collections.Generic.List[object]
            if ($parent.libraries) { foreach ($l in $parent.libraries) { $libs.Add($l) } }
            if ($json.libraries)   { foreach ($l in $json.libraries)   { $libs.Add($l) } }
            # attach merged
            $json | Add-Member -NotePropertyName "_mergedLibs" -NotePropertyValue $libs -Force

            # arguments: merge jvm + game arrays
            if (-not $json.arguments) {
                $json | Add-Member -NotePropertyName "arguments" -NotePropertyValue ([PSCustomObject]@{ game = @(); jvm = @() }) -Force
            }
            if (-not $json.arguments.game) { $json.arguments | Add-Member -NotePropertyName "game" -NotePropertyValue @() -Force }
            if (-not $json.arguments.jvm)  { $json.arguments | Add-Member -NotePropertyName "jvm"  -NotePropertyValue @() -Force }

            $mergedJvm = New-Object System.Collections.Generic.List[object]
            if ($parent.arguments -and $parent.arguments.jvm) { foreach ($a in $parent.arguments.jvm) { $mergedJvm.Add($a) } }
            foreach ($a in $json.arguments.jvm) { $mergedJvm.Add($a) }

            $mergedGame = New-Object System.Collections.Generic.List[object]
            if ($parent.arguments -and $parent.arguments.game) { foreach ($a in $parent.arguments.game) { $mergedGame.Add($a) } }
            foreach ($a in $json.arguments.game) { $mergedGame.Add($a) }

            $json.arguments.jvm  = $mergedJvm
            $json.arguments.game = $mergedGame

            # inherit missing fields from parent (use Add-Member since PSCustomObject
            # cannot be assigned properties it doesn't already have).
            if (-not $json.mainClass -and $parent.mainClass) { $json | Add-Member -NotePropertyName mainClass -NotePropertyValue $parent.mainClass -Force }
            if (-not $json.assetIndex -and $parent.assetIndex) { $json | Add-Member -NotePropertyName assetIndex -NotePropertyValue $parent.assetIndex -Force }
            if (-not $json.assets -and $parent.assets) { $json | Add-Member -NotePropertyName assets -NotePropertyValue $parent.assets -Force }
            if (-not $json.minecraftArguments -and $parent.minecraftArguments) { $json | Add-Member -NotePropertyName minecraftArguments -NotePropertyValue $parent.minecraftArguments -Force }
            if (-not $json.javaVersion -and $parent.javaVersion) { $json | Add-Member -NotePropertyName javaVersion -NotePropertyValue $parent.javaVersion -Force }
        }
    } else {
        if ($json.libraries) {
            $libs = New-Object System.Collections.Generic.List[object]
            foreach ($l in $json.libraries) { $libs.Add($l) }
            $json | Add-Member -NotePropertyName "_mergedLibs" -NotePropertyValue $libs -Force
        }
    }
    return $json
}

# =========================================================
#  Java
# =========================================================
# Read java's version via the .NET Process API, so we are immune to
# PowerShell turning native stderr into a terminating error (which happens
# because $ErrorActionPreference = 'Stop'). Returns the major version (0 on failure).
function Get-JavaVersion([string]$exe) {
    if (-not (Test-Path $exe)) { return 0 }
    try {
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = $exe
        $psi.Arguments = '-version'
        $psi.RedirectStandardError = $true
        $psi.RedirectStandardOutput = $true
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $proc = [System.Diagnostics.Process]::Start($psi)
        $output = $proc.StandardError.ReadToEnd()
        if (-not $proc.WaitForExit(5000)) { try { $proc.Kill() } catch {} }
        # Java 8 reports as "1.8.0_xxx" (the only version keeping the 1. prefix);
        # Java 9+ reports as "9", "17.0.x", "21.0.x" directly.
        if ($output -match 'version "1\.8') { return 8 }
        if ($output -match 'version "(\d+)') { return [int]$Matches[1] }
    } catch { }
    return 0
}

function Find-Java([int]$minVersion = 17) {
    # Scan every portable jdk-* folder and remember candidates.
    $jdkDirs = @(Get-ChildItem -Path $workDir -Directory -Filter 'jdk-*' -ErrorAction SilentlyContinue)
    $candidates = @()
    foreach ($d in $jdkDirs) {
        $exe = Join-Path $d.FullName 'bin\java.exe'
        if (Test-Path $exe) {
            $major = Get-JavaVersion $exe
            if ($major -gt 0) { $candidates += [PSCustomObject]@{ Major = $major; Exe = $exe } }
        }
    }

    # Prefer an EXACT match to the requested major version (e.g. Java 8 for 1.7.10),
    # because old Minecraft cannot run on newer JVMs even if they meet the "minimum".
    # Fall back to the lowest available Java that still satisfies minVersion.
    $exact = @($candidates | Where-Object { $_.Major -eq $minVersion })
    if ($exact.Count -gt 0) {
        Write-Host "Found portable Java $($exact[0].Major) (exact match for $minVersion)" -ForegroundColor Gray
        return $exact[0].Exe
    }

    $qualified = @($candidates | Where-Object { $_.Major -ge $minVersion } | Sort-Object Major)
    if ($qualified.Count -gt 0) {
        $pick = $qualified[0]
        Write-Host "Found portable Java $($pick.Major) (need $minVersion+)" -ForegroundColor Gray
        return $pick.Exe
    }

    # System Java fallback
    $sysExe = Get-Command java -ErrorAction SilentlyContinue
    if ($sysExe) {
        $major = Get-JavaVersion $sysExe.Source
        if ($major -ge $minVersion) {
            Write-Host "Found system Java $major (need $minVersion+)" -ForegroundColor Gray
            return 'java'
        } else {
            Write-Host "System Java is $major, need $minVersion+" -ForegroundColor Red
        }
    } else {
        Write-Host "System Java not found" -ForegroundColor Red
    }
    return $null
}

# =========================================================
#  Natives extraction
# =========================================================
Add-Type -AssemblyName System.IO.Compression.FileSystem
function Extract-Natives($nativeJars, $nativesDir) {
    if (Test-Path $nativesDir) { Remove-Item $nativesDir -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $nativesDir | Out-Null
    foreach ($jar in $nativeJars) {
        if (-not $jar -or -not (Test-Path $jar)) { continue }
        try {
            $zip = [System.IO.Compression.ZipFile]::OpenRead($jar)
            foreach ($entry in $zip.Entries) {
                $name = $entry.FullName
                # skip directories (end with /) and META-INF
                if ($name -match '/$') { continue }
                if ($name -match '^META-INF/') { continue }
                $dest = Join-Path $nativesDir $name
                $parent = Split-Path $dest -Parent
                if ($parent -and -not (Test-Path $parent)) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
                [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $dest, $true)
            }
            $zip.Dispose()
        } catch {
            Write-Host "  Warn: cannot extract $jar" -ForegroundColor Yellow
        }
    }
}

# =========================================================
#  LAUNCH (vanilla + Forge via JSON parsing)
# =========================================================
function Run-Launch {
    Write-Host "`n--- Launch Minecraft ---" -ForegroundColor Cyan

    $versionsDir = Join-Path $workDir "versions"
    if (-not (Test-Path $versionsDir)) {
        Write-Host "Folder 'versions' not found. Install a version first." -ForegroundColor Red
        Pause-Host; return
    }

    # installed versions: folders containing <name>.json (jar optional for Forge)
    $versionFolders = @(Get-ChildItem -Path $versionsDir -Directory | Where-Object {
        (Test-Path (Join-Path $_.FullName "$($_.Name).json")) -or (Test-Path (Join-Path $_.FullName "$($_.Name).jar"))
    })

    if ($versionFolders.Count -eq 0) {
        Write-Host "No installed versions found." -ForegroundColor Red
        Pause-Host; return
    }

    Write-Host "`nSelect version to launch:" -ForegroundColor Cyan
    $i = 1
    foreach ($v in $versionFolders) {
        $tag = if ($v.Name -match 'forge') { " [Forge]" } else { "" }
        Write-Host ("{0,2}. {1}{2}" -f $i, $v.Name, $tag)
        $i++
    }
    Write-Host " 0. Back"

    $choice = Read-Host "`nEnter number"
    if ($choice -eq '0') { return }
    $idx = 0
    if (-not ([int]::TryParse($choice, [ref]$idx))) { Write-Host "Invalid number" -ForegroundColor Red; Pause-Host; return }
    $idx -= 1
    if ($idx -lt 0 -or $idx -ge $versionFolders.Count) {
        Write-Host "Invalid number" -ForegroundColor Red; Pause-Host; return
    }

    $selected = $versionFolders[$idx]
    $VERSION = $selected.Name
    Write-Host "`nLoading $VERSION ..." -ForegroundColor Green

    # ---- Load & resolve version JSON ----
    $json = Get-ResolvedVersionJson $VERSION $versionsDir
    if (-not $json) {
        Write-Host "Cannot read version JSON: versions\$VERSION\$VERSION.json" -ForegroundColor Red
        Pause-Host; return
    }

    # ---- Required Java version ----
    $minJava = 17
    if ($json.javaVersion -and $json.javaVersion.majorVersion) { $minJava = [int]$json.javaVersion.majorVersion }
    $javaExe = Find-Java $minJava
    if (-not $javaExe) {
        Write-Host "ERROR: Java $minJava+ not found." -ForegroundColor Red
        Write-Host "Place portable Java in: $workDir\jdk-$minJava" -ForegroundColor Yellow
        Pause-Host; return
    }

    # ---- Libraries: classpath + native jars ----
    $libDir = Join-Path $workDir "libraries"
    $os = Get-OsName
    $arch = if ([Environment]::Is64BitOperatingSystem) { '64' } else { '32' }

    # Enabled launcher features. Empty = normal offline launch: this correctly
    # excludes feature-gated args like --demo, --width/--height, --quickPlay*.
    $enabledFeatures = @{}

    $classpathList = New-Object System.Collections.Generic.List[string]
    $nativeJarList = New-Object System.Collections.Generic.List[string]

    # client jar of this version goes first (if it has one)
    $clientJar = Join-Path $selected.FullName "$VERSION.jar"
    if (Test-Path $clientJar) { $classpathList.Add($clientJar) | Out-Null }

    # Determine main class up front (needed to decide classpath strategy)
    $mainClass = if ($json.mainClass) { $json.mainClass } else { 'net.minecraft.client.main.Main' }
    $isForge = $mainClass -like '*BootstrapLauncher*'

    # For inheriting versions, the game classes live in the PARENT's client jar.
    # BUT Forge (BootstrapLauncher) loads MC classes itself via --launchTarget,
    # so adding the parent jar would create a conflicting auto-module "_1._20._1".
    if ($json.inheritsFrom -and -not $isForge) {
        $parentName = $json.inheritsFrom
        $parentJar = Join-Path $versionsDir "$parentName\$parentName.jar"
        if (Test-Path $parentJar) {
            $classpathList.Add($parentJar) | Out-Null
        } else {
            Write-Host "Warning: parent version '$parentName' jar missing." -ForegroundColor Yellow
        }
    }

    $libs = if ($json._mergedLibs) { $json._mergedLibs } elseif ($json.libraries) { $json.libraries } else { @() }

    # De-duplicate by library coordinates, keeping the LAST occurrence.
    # When a version inherits (e.g. Forge over vanilla), the child's libraries
    # are appended after the parent's and are meant to OVERRIDE them (e.g. Forge
    # ships guava 17.0 to replace vanilla's guava 15.0). Without de-dup, both
    # jars end up on the classpath and Java picks the wrong one (NoSuchMethodError).
    #
    # De-dup key = group:artifact plus classifier (if any), but WITHOUT version.
    # This way version overrides collapse (guava 15.0 vs 17.0 -> one), while
    # classifier variants stay distinct (lwjgl:3.3.1 base vs :natives-windows
    # are different jars and must both remain).
    $seenLibs = [ordered]@{}
    foreach ($lib in $libs) {
        $name = "$($lib.name)"
        $parts = $name -split ':'
        if ($parts.Count -ge 4) {
            # group:artifact:version:classifier -> key keeps classifier
            $dupKey = $parts[0] + ':' + $parts[1] + ':' + $parts[3]
        } elseif ($parts.Count -eq 3) {
            # group:artifact:version -> key drops version (overrides collapse)
            $dupKey = $parts[0] + ':' + $parts[1]
        } else {
            $dupKey = $name
        }
        $seenLibs[$dupKey] = $lib
    }
    $dedupedLibs = @($seenLibs.Values)

    foreach ($lib in $dedupedLibs) {
        if (-not (Test-RulesAllowed $lib.rules $enabledFeatures)) { continue }

        # Detect native libraries. Two formats exist:
        #  - Old (<1.19): library has a "natives" map -> classifier per OS
        #  - New (1.19+): each native is a separate entry whose name contains
        #    ":natives-<os>" (e.g. org.lwjgl:lwjgl:3.3.1:natives-windows)
        $libName = "$($lib.name)"
        $isNative = $false
        $classifier = $null
        if ($lib.natives -and $lib.natives.$os) {
            $isNative = $true
            $classifier = $lib.natives.$os.Replace('${arch}', $arch)
        } elseif ($libName -match ":natives-$os`b") {
            $isNative = $true
        }

        if ($isNative) {
            $np = Resolve-LibPath $lib $libDir $classifier
            if ($np -and (Test-Path $np)) { $nativeJarList.Add($np) | Out-Null }
        } else {
            $lp = Resolve-LibPath $lib $libDir $null
            if ($lp) { $classpathList.Add($lp) | Out-Null }
        }
    }

    # ---- Natives extraction ----
    $nativesDir = Join-Path $selected.FullName "natives-extracted"
    Write-Host "Extracting natives..." -ForegroundColor Gray
    Extract-Natives $nativeJarList $nativesDir

    # ---- Asset index ----
    $assetIndex = if ($json.assetIndex -and $json.assetIndex.id) { $json.assetIndex.id }
                  elseif ($json.assets) { $json.assets }
                  else { $VERSION }

    # ---- Build variable table ----
    $cpSep = ';'
    $vars = @{
        'auth_player_name'   = $script:USERNAME
        'version_name'       = $VERSION
        'game_directory'     = $workDir
        'assets_root'        = (Join-Path $workDir 'assets')
        'assets_index_name'  = $assetIndex
        'auth_uuid'          = $UUID
        'auth_access_token'  = $ACCESS_TOKEN
        'clientid'           = $UUID
        'auth_xuid'          = '0'
        'user_properties'    = '{}'
        'user_type'          = $USER_TYPE
        'version_type'       = $VERSION_TYPE
        'natives_directory' = $nativesDir
        'launcher_name'      = 'mc_console'
        'launcher_version'   = '1.0'
        'classpath'          = ($classpathList -join $cpSep)
        'classpath_separator'= $cpSep
        'library_directory'  = $libDir
    }

    # ---- JVM arguments ----
    $jvmArgs = New-Object System.Collections.Generic.List[string]
    $jvmArgs.Add("-Xms$script:RAM_MIN") | Out-Null
    $jvmArgs.Add("-Xmx$script:RAM_MAX") | Out-Null

    if ($json.arguments -and $json.arguments.jvm) {
        $extra = Expand-Arguments $json.arguments.jvm $vars $enabledFeatures
        foreach ($a in $extra) { $jvmArgs.Add($a) | Out-Null }
    } else {
        # legacy / fallback
        $jvmArgs.Add("-Djava.library.path=`"$nativesDir`"") | Out-Null
        $jvmArgs.Add("-cp") | Out-Null
        $jvmArgs.Add("`"$($vars['classpath'])`"") | Out-Null
    }

    # Forge (BootstrapLauncher) uses a module path (-p) instead of -cp, but its
    # BootstrapLauncher reads the legacy classpath from java.class.path to find
    # ModLauncher's service. The forge JSON omits -cp, so add it explicitly.
    if ($isForge) {
        # ensure java.library.path is set (forge JSON omits it, inherits from vanilla)
        $hasLibPath = $false
        foreach ($a in $jvmArgs) { if ($a -like '-Djava.library.path*') { $hasLibPath = $true; break } }
        if (-not $hasLibPath) { $jvmArgs.Add("-Djava.library.path=`"$nativesDir`"") | Out-Null }
        # add the legacy classpath explicitly so BootstrapLauncher can see ModLauncher etc.
        $jvmArgs.Add("-cp") | Out-Null
        $jvmArgs.Add($vars['classpath']) | Out-Null
    }

    # ---- Game arguments ----
    $gameArgs = New-Object System.Collections.Generic.List[string]
    if ($json.arguments -and $json.arguments.game) {
        $g = Expand-Arguments $json.arguments.game $vars $enabledFeatures
        foreach ($a in $g) { $gameArgs.Add($a) | Out-Null }
    } elseif ($json.minecraftArguments) {
        # legacy single string (1.12.2 and older)
        $legacyStr = Resolve-Placeholders $json.minecraftArguments $vars
        foreach ($tok in $legacyStr -split ' ') {
            if ($tok) { $gameArgs.Add($tok) | Out-Null }
        }
    } else {
        $gameArgs.AddRange(@(
            "--username", $script:USERNAME, "--version", $VERSION,
            "--gameDir", "`"$workDir`"",
            "--assetsDir", "`"$(Join-Path $workDir 'assets')`"",
            "--assetIndex", $assetIndex,
            "--uuid", $UUID, "--accessToken", $ACCESS_TOKEN,
            "--userType", $USER_TYPE, "--versionType", $VERSION_TYPE
        )) | Out-Null
    }

    # ---- Assemble & launch ----
    $allArgs = @($jvmArgs) + @($mainClass) + @($gameArgs)

    Write-Host "`nLaunching $VERSION ..." -ForegroundColor Green
    Write-Host "Java: $javaExe" -ForegroundColor Gray
    Write-Host "Args: $($allArgs -join ' ')" -ForegroundColor DarkGray

    try {
        & $javaExe $allArgs
    } catch {
        Write-Host "Launch error: $($_.Exception.Message)" -ForegroundColor Red
    }
    Pause-Host
}

# =========================================================
#  INSTALL (vanilla)
# =========================================================
function Select-RemoteVersion {
    Write-Host "`nFetching release versions from Mojang..." -ForegroundColor Cyan
    $manifest = Invoke-RestMethod "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
    $versions = @($manifest.versions | Where-Object { $_.type -eq 'release' -and $_.id -match '^1\.' } | Select-Object -First 20)

    $i = 1
    Write-Host "`nSelect version:" -ForegroundColor Yellow
    foreach ($v in $versions) {
        Write-Host "$i. $($v.id)"
        $i++
    }
    Write-Host "01. Enter custom version (e.g., 1.20.1)"
    Write-Host "0.  Back"

    $choice = Read-Host "`nEnter number"
    if ($choice -eq '01') {
        $ver = Read-Host "Enter version"
        return $ver.Trim()
    }
    if ($choice -eq '0') { return $null }

    $idx = 0
    if (-not ([int]::TryParse($choice, [ref]$idx))) { Write-Host "Invalid number!" -ForegroundColor Red; return $null }
    $idx -= 1
    if ($idx -ge 0 -and $idx -lt $versions.Count) { return $versions[$idx].id }
    Write-Host "Invalid number!" -ForegroundColor Red
    return $null
}

function Download-Version($version) {
    Write-Host "`n[1/4] Fetching manifest for $version..." -ForegroundColor Cyan
    $manifest = Invoke-RestMethod "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
    $verInfo = $manifest.versions | Where-Object { $_.id -eq $version } | Select-Object -First 1
    if (-not $verInfo) { throw "Version $version not found!" }

    $verJson = Invoke-RestMethod $verInfo.url
    $clientUrl = $verJson.downloads.client.url
    $clientSha1 = $verJson.downloads.client.sha1
    $libraries = $verJson.libraries
    $assetIndex = $verJson.assetIndex
    $assetsId = $assetIndex.id
    $assetsUrl = $assetIndex.url

    $versionDir = [System.IO.Path]::Combine($workDir, "versions", $version)
    New-Item -ItemType Directory -Force -Path $versionDir | Out-Null
    $libDir = [System.IO.Path]::Combine($workDir, "libraries")
    New-Item -ItemType Directory -Force -Path $libDir | Out-Null
    $assetsDir = [System.IO.Path]::Combine($workDir, "assets")
    $indexesDir = [System.IO.Path]::Combine($assetsDir, "indexes")
    New-Item -ItemType Directory -Force -Path $indexesDir | Out-Null
    $objectsDir = [System.IO.Path]::Combine($assetsDir, "objects")
    New-Item -ItemType Directory -Force -Path $objectsDir | Out-Null

    # version json
    $vJsonPath = [System.IO.Path]::Combine($versionDir, "$version.json")
    if (-not (Test-Path $vJsonPath)) {
        Invoke-WebRequest $verInfo.url -OutFile $vJsonPath -UseBasicParsing
    }

    # Client jar
    $clientJar = [System.IO.Path]::Combine($versionDir, "$version.jar")
    if (-not (Test-Path $clientJar)) {
        Write-Host "[2/4] Downloading client.jar..." -ForegroundColor Cyan
        Invoke-WebRequest $clientUrl -OutFile $clientJar -UseBasicParsing
        $sha = (Get-FileHash $clientJar -Algorithm SHA1).Hash.ToLower()
        if ($sha -ne $clientSha1) { Write-Host "Warning: SHA1 mismatch!" -ForegroundColor Yellow }
    } else {
        Write-Host "client.jar already exists" -ForegroundColor Gray
    }

    # Asset index
    $indexFile = [System.IO.Path]::Combine($indexesDir, "$assetsId.json")
    if (-not (Test-Path $indexFile)) {
        Write-Host "Downloading asset index..." -ForegroundColor Yellow
        Invoke-WebRequest $assetsUrl -OutFile $indexFile -UseBasicParsing
    }

    # Libraries
    Write-Host "[3/4] Downloading libraries..." -ForegroundColor Cyan
    $libCount = 0; $libTotal = $libraries.Count

    # Maven repos tried (in order) for libraries that declare only group:artifact:version
    # without a downloads.artifact (common in legacy/Forge 1.7.10 version JSONs).
    $mavenRepos = @(
        'https://libraries.minecraft.net',
        'https://maven.minecraftforge.net'
    )

    foreach ($lib in $libraries) {
        $libCount++
        $dl = $lib.downloads
        if ($dl -and $dl.artifact) {
            $target = [System.IO.Path]::Combine($libDir, $dl.artifact.path)
            if (-not (Test-Path $target)) {
                $targetDir = Split-Path $target -Parent
                New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
                try { Invoke-WebRequest $dl.artifact.url -OutFile $target -UseBasicParsing } catch { Write-Host "  Error: $($dl.artifact.path)" -ForegroundColor Red }
            }
        } elseif ($lib.name) {
            # Legacy library with no downloads field: build maven path + try multiple repos.
            $rel = Get-MavenRelPath $lib.name $null
            if ($rel) {
                $target = [System.IO.Path]::Combine($libDir, $rel)
                if (-not (Test-Path $target)) {
                    $targetDir = Split-Path $target -Parent
                    New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
                    $downloaded = $false
                    foreach ($repo in $mavenRepos) {
                        $url = "$repo/$rel"
                        try {
                            Invoke-WebRequest $url -OutFile $target -UseBasicParsing
                            $downloaded = $true
                            break
                        } catch { }
                    }
                    if (-not $downloaded) { Write-Host "  Missing: $($lib.name)" -ForegroundColor DarkYellow }
                }
            }
        }
        if ($dl -and $dl.classifiers) {
            foreach ($key in $dl.classifiers.PSObject.Properties.Name) {
                if ($key -match 'natives') {
                    $class = $dl.classifiers.$key
                    $target = [System.IO.Path]::Combine($libDir, $class.path)
                    if (-not (Test-Path $target)) {
                        $targetDir = Split-Path $target -Parent
                        New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
                        try { Invoke-WebRequest $class.url -OutFile $target -UseBasicParsing } catch { }
                    }
                }
            }
        }
        if ($libCount % 10 -eq 0 -or $libCount -eq $libTotal) {
            Write-Host "`r      Libraries: $libCount / $libTotal" -NoNewline
        }
    }
    Write-Host "`r      Libraries: $libTotal / $libTotal done" -ForegroundColor Green

    # Assets
    Write-Host "[4/4] Downloading assets (sounds, textures)..." -ForegroundColor Cyan
    $index = Get-Content $indexFile -Raw | ConvertFrom-Json
    $all = @($index.objects.PSObject.Properties)
    $total = $all.Count; $i = 0; $dl = 0; $skip = 0; $fail = 0
    foreach ($entry in $all) {
        $i++
        $hash = ([string]$entry.Value.hash).ToLowerInvariant()
        $size = [int64]$entry.Value.size
        $sub = $hash.Substring(0,2)
        $target = [System.IO.Path]::Combine($objectsDir, $sub, $hash)
        if ((Test-Path $target) -and ((Get-Item $target).Length -eq $size)) { $skip++; continue }
        $subDir = [System.IO.Path]::Combine($objectsDir, $sub)
        New-Item -ItemType Directory -Force -Path $subDir | Out-Null
        $url = "https://resources.download.minecraft.net/$sub/$hash"
        try {
            Invoke-WebRequest $url -OutFile "$target.part" -UseBasicParsing
            if ((Get-Item "$target.part").Length -ne $size) { throw "Size mismatch" }
            Move-Item "$target.part" $target -Force
            $dl++
        } catch {
            $fail++
            Remove-Item "$target.part" -Force -ErrorAction SilentlyContinue
        }
        if ($i % 100 -eq 0 -or $i -eq $total) {
            $pct = [math]::Round(($i / $total) * 100, 1)
            Write-Host "`r      $pct% | $i/$total | New: $dl | Cached: $skip | Errors: $fail" -NoNewline
        }
    }
    Write-Host "`n      Assets done! Total: $total, New: $dl, Cached: $skip, Errors: $fail" -ForegroundColor Green
    Write-Host "`n[DONE] Version $version fully downloaded to:`n$workDir" -ForegroundColor Green
}

function Run-Install {
    Write-Host "`n--- Install Minecraft (vanilla) ---" -ForegroundColor Cyan
    try {
        $version = Select-RemoteVersion
        if (-not $version) { return }
        Download-Version $version
    } catch {
        Write-Host "ERROR: $($_.Exception.Message)" -ForegroundColor Red
    }
    Pause-Host
}

# =========================================================
#  INSTALL FORGE
# =========================================================
function Compare-McVersion($a, $b) {
    # Tolerant comparison: numeric segments where possible, else lexical.
    # Handles non-pure-number MC tags like "1.7.10-pre4".
    $aa = "$a".Split('.'); $bb = "$b".Split('.')
    $max = [Math]::Max($aa.Count, $bb.Count)
    for ($i = 0; $i -lt $max; $i++) {
        $av = if ($i -lt $aa.Count) { $aa[$i] } else { '0' }
        $bv = if ($i -lt $bb.Count) { $bb[$i] } else { '0' }
        $an = 0; $bn = 0
        $aIsNum = [int]::TryParse($av, [ref]$an)
        $bIsNum = [int]::TryParse($bv, [ref]$bn)
        if ($aIsNum -and $bIsNum) {
            if ($an -ne $bn) { return ($an - $bn) }
        } else {
            # at least one is non-numeric: compare as strings (case-insensitive)
            $cmp = [string]::Compare($av, $bv, $true)
            if ($cmp -ne 0) { return $cmp }
        }
    }
    return 0
}

function Get-ForgeMetadata {
    # maven-metadata.xml is the canonical, always-available list of every build.
    Write-Host "Fetching Forge version list..." -ForegroundColor Cyan
    $resp = Invoke-WebRequest "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml" -UseBasicParsing
    [xml]$doc = $resp.Content
    $allVersions = @($doc.metadata.versioning.versions.version)

    # Group builds by Minecraft version (split "1.20.1-47.3.0" -> mc=1.20.1, forge=47.3.0)
    $byMc = @{}
    foreach ($v in $allVersions) {
        $dash = "$v".IndexOf('-')
        if ($dash -lt 0) { continue }
        $mc = "$v".Substring(0, $dash)
        $fg = "$v".Substring($dash + 1)
        if (-not $byMc.ContainsKey($mc)) { $byMc[$mc] = New-Object System.Collections.Generic.List[string] }
        $byMc[$mc].Add($fg) | Out-Null
    }

    # Best-effort recommended/latest labels (promotions_slim.json moved to files.minecraftforge.net).
    $promos = $null
    foreach ($url in @(
        "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
        "https://maven.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json"
    )) {
        try { $promos = (Invoke-RestMethod $url).promos; break } catch { }
    }

    return @{ byMc = $byMc; promos = $promos }
}

function Select-ForgeVersion {
    $meta = Get-ForgeMetadata
    $byMc = $meta.byMc; $promos = $meta.promos
    if ($byMc.Count -eq 0) { Write-Host "No Forge versions found." -ForegroundColor Red; return $null }

    # Unique MC versions, newest first. Only show real MC versions (1.x); exotic
    # coordinates still remain reachable via the custom-version option.
    $mcList = @($byMc.Keys | Where-Object { $_ -match '^1\.' })
    $mcSorted = [System.Collections.Generic.List[string]]::new()
    foreach ($m in $mcList) { $mcSorted.Add($m) | Out-Null }
    $mcSorted.Sort([Comparison[string]]{ param($a, $b) (Compare-McVersion $b $a) })

    # Keep the list manageable: show the most recent ones
    $showCount = [Math]::Min(25, $mcSorted.Count)
    Write-Host "`nSelect Minecraft version for Forge (newest first):" -ForegroundColor Yellow
    for ($i = 0; $i -lt $showCount; $i++) {
        $mc = $mcSorted[$i]
        $rec = if ($promos) { $promos."$mc-recommended" } else { $null }
        $lst = if ($promos) { $promos."$mc-latest" } else { $null }
        $buildCount = $byMc[$mc].Count
        $shown = if ($rec) { "recommended $rec" } elseif ($lst) { "latest $lst" } else { "$buildCount builds" }
        Write-Host ("{0,2}. {1}  ({2})" -f ($i + 1), $mc, $shown)
    }
    Write-Host "01. Enter custom Minecraft version (e.g., 1.20.1)"
    Write-Host "0.  Back"

    $choice = Read-Host "`nEnter number"
    if ($choice -eq '0') { return $null }

    $mc = $null
    if ($choice -eq '01') {
        $mc = (Read-Host "Enter Minecraft version").Trim()
        if (-not $byMc.ContainsKey($mc)) {
            Write-Host "No Forge builds available for $mc" -ForegroundColor Red
            return $null
        }
    } else {
        $idx = 0
        if (-not ([int]::TryParse($choice, [ref]$idx))) { Write-Host "Invalid number!" -ForegroundColor Red; return $null }
        $idx -= 1
        if ($idx -lt 0 -or $idx -ge $showCount) { Write-Host "Invalid number!" -ForegroundColor Red; return $null }
        $mc = $mcSorted[$idx]
    }

    # Determine which Forge build. Default = recommended (or latest); else let user pick from the list.
    $builds = $byMc[$mc]
    $forge = $null
    if ($promos) { $forge = $promos."$mc-recommended" }
    if (-not $forge -and $promos) { $forge = $promos."$mc-latest" }
    if (-not $forge -or ($builds -notcontains $forge)) { $forge = $builds[0] }   # first = newest in metadata

    Write-Host "`nMinecraft $mc has $($builds.Count) Forge build(s)." -ForegroundColor Gray
    Write-Host "Default build: $forge" -ForegroundColor Green
    $ans = Read-Host "Use this build? (Y = yes / n = choose from list)"
    if ($ans -eq 'n' -or $ans -eq 'N') {
        Write-Host "`nSelect Forge build for ${mc}:" -ForegroundColor Yellow
        # show newest first (metadata is newest-first already)
        for ($i = 0; $i -lt $builds.Count; $i++) {
            Write-Host ("{0,3}. {1}" -f ($i + 1), $builds[$i])
        }
        Write-Host " 0. Back"
        $bchoice = Read-Host "`nEnter number"
        if ($bchoice -eq '0') { return $null }
        $bidx = 0
        if (-not ([int]::TryParse($bchoice, [ref]$bidx))) { Write-Host "Invalid number!" -ForegroundColor Red; return $null }
        $bidx -= 1
        if ($bidx -lt 0 -or $bidx -ge $builds.Count) { Write-Host "Invalid number!" -ForegroundColor Red; return $null }
        $forge = $builds[$bidx]
    }

    return @{ mc = $mc; forge = $forge }
}

function Run-InstallForge {
    Write-Host "`n--- Install Forge ---" -ForegroundColor Cyan
    try {
        $sel = Select-ForgeVersion
        if (-not $sel) { return }
        $mc = $sel.mc; $forge = $sel.forge
        $fullVersion = "$mc-$forge"
        Write-Host "`nSelected Forge $fullVersion" -ForegroundColor Green

        # Java to run the installer GUI
        $javaExe = Find-Java 17
        if (-not $javaExe) {
            Write-Host "ERROR: Java 17+ needed to run the Forge installer." -ForegroundColor Red
            Write-Host "Place portable Java in: $workDir\jdk-17" -ForegroundColor Yellow
            Pause-Host; return
        }

        # ensure parent vanilla version is present (Forge inherits from it)
        $parentJson = Join-Path $workDir "versions\$mc\$mc.json"
        if (-not (Test-Path $parentJson)) {
            Write-Host "Parent vanilla version $mc is missing. Forge needs it." -ForegroundColor Yellow
            $ans = Read-Host "Download vanilla $mc now? (y/n)"
            if ($ans -eq 'y' -or $ans -eq 'Y') {
                Download-Version $mc
            } else {
                Write-Host "Cannot install Forge without vanilla $mc." -ForegroundColor Red
                Pause-Host; return
            }
        }

        # Forge installer requires a launcher_profiles.json to be present in the
        # target dir (it looks for an "official launcher" marker). Create a minimal
        # one so the installer accepts our directory instead of erroring out.
        $profilesFile = Join-Path $workDir "launcher_profiles.json"
        if (-not (Test-Path $profilesFile)) {
            $profileJson = @{
                profiles = @{
                    mc_console = @{
                        name = "mc_console"
                        type = "latest-release"
                        icon = "Grass"
                        lastVersionId = "latest-release"
                        gameDir = $workDir
                    }
                }
                selectedProfile = "mc_console"
                clientToken = "mc-console-offline-0001"
                authenticationDatabase = @{}
                launcherVersion = @{ name = "2.1.0"; format = 0 }
            } | ConvertTo-Json -Depth 6
            [System.IO.File]::WriteAllText($profilesFile, $profileJson)
            Write-Host "Created launcher_profiles.json (Forge installer requirement)." -ForegroundColor Gray
        }

        # download installer jar
        $installerName = "forge-$fullVersion-installer.jar"
        $installersDir = Join-Path $workDir "installers"
        New-Item -ItemType Directory -Force -Path $installersDir | Out-Null
        $installerPath = Join-Path $installersDir $installerName

        $url = "https://maven.minecraftforge.net/net/minecraftforge/forge/$fullVersion/$installerName"
        Write-Host "Downloading Forge installer..." -ForegroundColor Cyan
        Invoke-WebRequest $url -OutFile $installerPath -UseBasicParsing
        Write-Host "Saved: $installerPath" -ForegroundColor Gray

        # open installer GUI with the working dir set to our game dir, so the
        # installer opens with $workDir pre-selected instead of %APPDATA%\.minecraft.
        Write-Host "`nOpening Forge installer window..." -ForegroundColor Green
        Write-Host '  -> Select "Install client" and press OK' -ForegroundColor Yellow
        Write-Host "  -> Make sure the path is: $workDir" -ForegroundColor Yellow

        $startInfo = New-Object System.Diagnostics.ProcessStartInfo
        $startInfo.FileName = $javaExe
        $startInfo.Arguments = "-jar `"$installerPath`""
        $startInfo.WorkingDirectory = $workDir
        $startInfo.UseShellExecute = $false
        $proc = [System.Diagnostics.Process]::Start($startInfo)
        $proc.WaitForExit()

        Write-Host "`nForge installer finished." -ForegroundColor Green
        Write-Host "Forge version should now appear in the launch menu." -ForegroundColor Cyan
    } catch {
        Write-Host "ERROR: $($_.Exception.Message)" -ForegroundColor Red
    }
    Pause-Host
}

# =========================================================
#  INSTALL JAVA (portable, from Adoptium / Eclipse Temurin)
# =========================================================
function Run-InstallJava {
    Write-Host "`n--- Install portable Java ---" -ForegroundColor Cyan
    Write-Host "Choose Java version:" -ForegroundColor Yellow
    Write-Host " 1. Java 21  (LTS, required for Minecraft 1.20.5+)"
    Write-Host " 2. Java 17  (LTS, recommended for Minecraft 1.17 - 1.20.4)"
    Write-Host " 3. Java 8   (legacy, required for Minecraft 1.16.5 and older)"
    Write-Host " 0.  Back"
    $choice = Read-Host "`nEnter number"
    if ($choice -eq '0') { return }

    $major = switch ($choice) {
        "1" { 21 }
        "2" { 17 }
        "3" { 8 }
        default { Write-Host "Invalid choice" -ForegroundColor Red; Pause-Host; return }
    }

    # check if already installed
    $existing = @(Resolve-Path -Path (Join-Path $workDir "jdk-$major*\bin\java.exe") -ErrorAction SilentlyContinue)
    if ($existing.Count -gt 0) {
        Write-Host "Java $major already installed: $($existing[0])" -ForegroundColor Green
        $ans = Read-Host "Reinstall? (y/n)"
        if ($ans -ne 'y' -and $ans -ne 'Y') { return }
    }

    $url = "https://api.adoptium.net/v3/binary/latest/$major/ga/windows/x64/jdk/hotspot/normal/eclipse"
    Write-Host "`nDownloading Java $major (Eclipse Temurin)..." -ForegroundColor Cyan
    Write-Host "  $url" -ForegroundColor DarkGray

    $zipPath = Join-Path $workDir "java-$major-download.zip"
    try {
        Invoke-WebRequest $url -OutFile $zipPath -UseBasicParsing
    } catch {
        Write-Host "Download failed: $($_.Exception.Message)" -ForegroundColor Red
        Pause-Host; return
    }

    Write-Host "Extracting..." -ForegroundColor Cyan
    $tempExtract = Join-Path $workDir "java-extract-$major"
    if (Test-Path $tempExtract) { Remove-Item $tempExtract -Recurse -Force }
    Expand-Archive -Path $zipPath -DestinationPath $tempExtract -Force

    # find the jdk folder inside (contains bin\java.exe)
    $jdkFolder = Get-ChildItem -Path $tempExtract -Directory | Where-Object {
        Test-Path (Join-Path $_.FullName "bin\java.exe")
    } | Select-Object -First 1
    if (-not $jdkFolder) {
        Write-Host "Extraction failed: java.exe not found in archive." -ForegroundColor Red
        Remove-Item $zipPath -Force -ErrorAction SilentlyContinue
        Pause-Host; return
    }

    # rename to jdk-<version>, replacing any existing one
    $targetName = "jdk-$major"
    $targetPath = Join-Path $workDir $targetName
    if (Test-Path $targetPath) { Remove-Item $targetPath -Recurse -Force }
    Move-Item $jdkFolder.FullName $targetPath
    Remove-Item $tempExtract -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item $zipPath -Force -ErrorAction SilentlyContinue

    # verify
    $javaExe = Join-Path $targetPath "bin\java.exe"
    if (Test-Path $javaExe) {
        $majorCheck = Get-JavaVersion $javaExe
        Write-Host ""
        Write-Host "[DONE] Java $majorCheck installed to: $targetPath" -ForegroundColor Green
    } else {
        Write-Host "Something went wrong, java.exe missing." -ForegroundColor Red
    }
    Pause-Host
}

# =========================================================
#  SETTINGS
# =========================================================
function Convert-RamToMb($value) {
    # accepts "4G", "1024M", "2048", "2.5G" -> integer megabytes
    $v = "$value".Trim().ToUpper()
    if ($v -match '^(\d+(?:\.\d+)?)G$') { return [int]([double]$Matches[1] * 1024) }
    if ($v -match '^(\d+(?:\.\d+)?)M$') { return [int][double]$Matches[1] }
    if ($v -match '^(\d+)$')            { return [int]$Matches[1] }
    return -1
}

function Format-Mb($mb) {
    if ($mb -ge 1024 -and ($mb % 1024) -eq 0) { return "$([math]::Floor($mb / 1024))G" }
    return "${mb}M"
}

function Run-Settings {
    while ($true) {
        Clear-Host
        Write-Host "================================" -ForegroundColor Cyan
        Write-Host "          Settings" -ForegroundColor Cyan
        Write-Host "================================" -ForegroundColor Cyan
        Write-Host "Memory allocated to Minecraft:" -ForegroundColor Yellow
        Write-Host "  Minimum: $($script:RAM_MIN)"
        Write-Host "  Maximum: $($script:RAM_MAX)"
        Write-Host ""
        Write-Host "In-game username:" -ForegroundColor Yellow
        Write-Host "  $($script:USERNAME)"
        Write-Host ""
        Write-Host "Content index URL (mods/resourcepacks/shaders):" -ForegroundColor Yellow
        $urlDisplay = if ($script:contentIndexUrl) { $script:contentIndexUrl } else { "(not set)" }
        Write-Host "  $urlDisplay"
        Write-Host ""
        Write-Host "  Launcher version: v$APP_VERSION"
        Write-Host ""
        Write-Host "Memory presets:" -ForegroundColor Yellow
        Write-Host " 1. 2G   / 4G     (default, for 8GB+ RAM)"
        Write-Host " 2. 4G   / 6G"
        Write-Host " 3. 4G   / 8G     (recommended for mods)"
        Write-Host " 4. 8G   / 12G    (heavy modpacks)"
        Write-Host " 5. 8G   / 16G"
        Write-Host " 6. Custom RAM values"
        Write-Host " 7. Set content index URL"
        Write-Host " 8. Change username"
        Write-Host " 9. Check for updates"
        Write-Host " 0. Back"
        $choice = Read-Host "`nEnter number"
        switch ($choice) {
            "1" { $script:RAM_MIN = "2G"; $script:RAM_MAX = "4G" }
            "2" { $script:RAM_MIN = "4G"; $script:RAM_MAX = "6G" }
            "3" { $script:RAM_MIN = "4G"; $script:RAM_MAX = "8G" }
            "4" { $script:RAM_MIN = "8G"; $script:RAM_MAX = "12G" }
            "5" { $script:RAM_MIN = "8G"; $script:RAM_MAX = "16G" }
            "6" {
                $minInput = Read-Host "Enter MINIMUM RAM (e.g. 2G, 2048M)"
                $maxInput = Read-Host "Enter MAXIMUM RAM (e.g. 4G, 4096M)"
                $minMb = Convert-RamToMb $minInput
                $maxMb = Convert-RamToMb $maxInput
                if ($minMb -le 0 -or $maxMb -le 0) {
                    Write-Host "Invalid value. Use formats like 2G or 2048M." -ForegroundColor Red
                    Pause-Host; continue
                }
                if ($minMb -gt $maxMb) {
                    Write-Host "Minimum cannot be greater than maximum." -ForegroundColor Red
                    Pause-Host; continue
                }
                if ($minMb -lt 512) {
                    Write-Host "Minimum 512M recommended." -ForegroundColor Yellow
                }
                $script:RAM_MIN = Format-Mb $minMb
                $script:RAM_MAX = Format-Mb $maxMb
                Save-Settings
                Write-Host ""
                Write-Host "Memory set to: MIN $($script:RAM_MIN) / MAX $($script:RAM_MAX)" -ForegroundColor Green
                Write-Host "(saved to mc_console_settings.json)" -ForegroundColor Gray
                Pause-Host; continue
            }
            "7" {
                Write-Host ""
                Write-Host "Paste the URL of your content index JSON." -ForegroundColor Yellow
                Write-Host "(Use a direct/raw link, e.g. https://raw.githubusercontent.com/.../index.json)" -ForegroundColor Gray
                $url = (Read-Host "URL").Trim()
                if ($url) {
                    $script:contentIndexUrl = $url
                    Save-Settings
                    Write-Host ""
                    Write-Host "Content index URL saved." -ForegroundColor Green
                } else {
                    Write-Host "Empty input, URL unchanged." -ForegroundColor Yellow
                }
                Pause-Host; continue
            }
            "8" {
                Write-Host ""
                Write-Host "Enter your in-game username (3-16 characters, letters/numbers/_)." -ForegroundColor Yellow
                $name = (Read-Host "Username").Trim()
                if (-not $name) {
                    Write-Host "Empty input, username unchanged." -ForegroundColor Yellow
                    Pause-Host; continue
                }
                if ($name.Length -lt 3 -or $name.Length -gt 16) {
                    Write-Host "Username must be 3-16 characters." -ForegroundColor Red
                    Pause-Host; continue
                }
                if ($name -notmatch '^[A-Za-z0-9_]+$') {
                    Write-Host "Only letters, numbers and underscore are allowed." -ForegroundColor Red
                    Pause-Host; continue
                }
                $script:USERNAME = $name
                Save-Settings
                Write-Host ""
                Write-Host "Username set to: $($script:USERNAME)" -ForegroundColor Green
                Write-Host "(saved to mc_console_settings.json)" -ForegroundColor Gray
                Pause-Host; continue
            }
            "9" {
                Write-Host ""
                Write-Host "Checking for updates..." -ForegroundColor Cyan
                $latest = Get-LatestVersion
                if (-not $latest) {
                    Write-Host "Could not reach the update server. Check your internet connection." -ForegroundColor Red
                    Pause-Host; continue
                }
                $cmp = Compare-Version $latest $APP_VERSION
                if ($cmp -le 0) {
                    Write-Host "You are on the latest version (v$APP_VERSION)." -ForegroundColor Green
                    Pause-Host; continue
                }
                Write-Host "New version available: v$APP_VERSION -> v$latest" -ForegroundColor Yellow
                $ans = Read-Host "Download and install now? (y/n)"
                if ($ans -eq 'y' -or $ans -eq 'Y') {
                    if (Invoke-SelfUpdate) {
                        Write-Host ""
                        Write-Host "Update installed successfully." -ForegroundColor Green
                        Write-Host "Restart the launcher to use the new version." -ForegroundColor Cyan
                        Read-Host "Press Enter to exit"
                        exit 0
                    }
                }
                Pause-Host; continue
            }
            "0" { return }
            default {
                Write-Host "Invalid choice" -ForegroundColor Red
                Start-Sleep -Seconds 1
                continue
            }
        }
        # For the memory presets (1-5): show a confirmation after applying.
        Save-Settings
        Write-Host ""
        Write-Host "Memory set to: MIN $($script:RAM_MIN) / MAX $($script:RAM_MAX)" -ForegroundColor Green
        Write-Host "(saved to mc_console_settings.json)" -ForegroundColor Gray
        Pause-Host
    }
}

# =========================================================
#  DOWNLOAD CONTENT (mods / resourcepacks / shaderpacks)
# =========================================================
# Map content category -> target subfolder under the game directory.
$script:contentCategories = @{
    'mods'         = 'mods'
    'resourcepacks' = 'resourcepacks'
    'shaderpacks'  = 'shaderpacks'
}

function Run-DownloadContent {
    Write-Host "`n--- Download content ---" -ForegroundColor Cyan

    # 1. Need an index URL configured.
    if (-not $script:contentIndexUrl) {
        Write-Host "Content index URL is not set." -ForegroundColor Red
        Write-Host "Configure it via Settings (5) -> Set content index URL (7)." -ForegroundColor Yellow
        Pause-Host; return
    }

    # 2. Fetch and parse the index.
    Write-Host "Fetching content index..." -ForegroundColor Cyan
    $index = $null
    try {
        $index = Invoke-RestMethod $script:contentIndexUrl -UseBasicParsing
    } catch {
        Write-Host "Failed to fetch index: $($_.Exception.Message)" -ForegroundColor Red
        Pause-Host; return
    }
    if (-not $index -or -not $index.versions) {
        Write-Host "No content found in index (expected a 'versions' object)." -ForegroundColor Red
        Pause-Host; return
    }

    # Collect versions that have at least one file in a known category.
    $versionsList = New-Object System.Collections.Generic.List[object]
    foreach ($prop in $index.versions.PSObject.Properties) {
        $mc = $prop.Name
        $entry = $prop.Value
        $catCounts = @{}
        foreach ($cat in $script:contentCategories.Keys) {
            $files = $entry.$cat
            if ($files -and @($files).Count -gt 0) { $catCounts[$cat] = @($files).Count }
        }
        if ($catCounts.Count -gt 0) {
            $versionsList.Add([PSCustomObject]@{ Name = $mc; Entry = $entry; Cats = $catCounts }) | Out-Null
        }
    }
    if ($versionsList.Count -eq 0) {
        Write-Host "Index has no downloadable files in known categories (mods/resourcepacks/shaderpacks)." -ForegroundColor Red
        Pause-Host; return
    }

    # ---- Level 1: pick a Minecraft version ----
    Write-Host "`nVersions available in index:" -ForegroundColor Yellow
    for ($i = 0; $i -lt $versionsList.Count; $i++) {
        $v = $versionsList[$i]
        $summary = ($v.Cats.Keys | ForEach-Object { "$($v.Cats[$_]) $_" }) -join ', '
        Write-Host ("{0,2}. {1}   ({2})" -f ($i + 1), $v.Name, $summary)
    }
    Write-Host " 0. Back"
    $choice = Read-Host "`nEnter number"
    if ($choice -eq '0') { return }
    $idx = 0
    if (-not ([int]::TryParse($choice, [ref]$idx))) { Write-Host "Invalid number" -ForegroundColor Red; Pause-Host; return }
    $idx -= 1
    if ($idx -lt 0 -or $idx -ge $versionsList.Count) { Write-Host "Invalid number" -ForegroundColor Red; Pause-Host; return }

    $selectedVer = $versionsList[$idx]

    # ---- Level 2: pick a category ----
    $availableCats = @($selectedVer.Cats.Keys | Sort-Object)
    Write-Host "`nSelect category for $($selectedVer.Name):" -ForegroundColor Yellow
    for ($i = 0; $i -lt $availableCats.Count; $i++) {
        $c = $availableCats[$i]
        Write-Host ("{0,2}. {1,-16} ({2} files)" -f ($i + 1), $c, $selectedVer.Cats[$c])
    }
    Write-Host " 0. Back"
    $choice = Read-Host "`nEnter number"
    if ($choice -eq '0') { return }
    $idx = 0
    if (-not ([int]::TryParse($choice, [ref]$idx))) { Write-Host "Invalid number" -ForegroundColor Red; Pause-Host; return }
    $idx -= 1
    if ($idx -lt 0 -or $idx -ge $availableCats.Count) { Write-Host "Invalid number" -ForegroundColor Red; Pause-Host; return }

    $category = $availableCats[$idx]
    $files = @($selectedVer.Entry.$category)

    # ---- Level 3: pick file(s) ----
    Write-Host "`nSelect $category for $($selectedVer.Name):" -ForegroundColor Yellow
    for ($i = 0; $i -lt $files.Count; $i++) {
        $f = $files[$i]
        $sz = if ($f.size) { $f.size } else { "" }
        Write-Host ("{0,2}. {1}   ({2})" -f ($i + 1), $f.name, $sz)
    }
    Write-Host " A. Download ALL ($($files.Count) files)"
    Write-Host " 0. Back"
    $choice = Read-Host "`nEnter number or 'A' for all"
    if ($choice -eq '0') { return }

    $toDownload = New-Object System.Collections.Generic.List[object]
    if ($choice -eq 'A' -or $choice -eq 'a') {
        foreach ($f in $files) { $toDownload.Add($f) | Out-Null }
    } else {
        # allow comma-separated list, e.g. "1,3"
        $nums = $choice -split ',' | ForEach-Object { $_.Trim() }
        foreach ($n in $nums) {
            $ni = 0
            if ([int]::TryParse($n, [ref]$ni)) {
                $ni -= 1
                if ($ni -ge 0 -and $ni -lt $files.Count) { $toDownload.Add($files[$ni]) | Out-Null }
            }
        }
    }
    if ($toDownload.Count -eq 0) { Write-Host "Nothing selected." -ForegroundColor Yellow; Pause-Host; return }

    # ---- Download ----
    $targetDir = Join-Path $workDir $script:contentCategories[$category]
    New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
    Write-Host "`nDownloading $($toDownload.Count) file(s) to: $targetDir" -ForegroundColor Cyan

    $ok = 0; $fail = 0; $skip = 0
    $i = 0
    foreach ($f in $toDownload) {
        $i++
        $name = $f.name
        $url = $f.url
        $target = Join-Path $targetDir $name
        if (-not $url) { Write-Host ("  [{0}/{1}] {2} - no URL, skipped" -f $i, $toDownload.Count, $name) -ForegroundColor Yellow; $skip++; continue }

        Write-Host ("  [{0}/{1}] {2} ... " -f $i, $toDownload.Count, $name) -NoNewline
        try {
            Invoke-WebRequest $url -OutFile $target -UseBasicParsing
            Write-Host "OK" -ForegroundColor Green
            $ok++
        } catch {
            Write-Host "FAILED" -ForegroundColor Red
            Write-Host "    $($_.Exception.Message)" -ForegroundColor DarkGray
            $fail++
        }
    }

    Write-Host ""
    Write-Host "Done: $ok downloaded, $skip skipped, $fail failed." -ForegroundColor $(if ($fail -eq 0) { 'Green' } else { 'Yellow' })
    Pause-Host
}

# =========================================================
#  INSTALL OPTIFINE
# =========================================================
# Parse optifine.net/downloads to list available builds grouped by MC version,
# then resolve the direct download URL (optifine.net gates downloads behind an
# ad page; the real link lives on that ad page as downloadx?f=...&x=<token>).
function Get-OptiFineVersions {
    Write-Host "Fetching OptiFine version list..." -ForegroundColor Cyan
    $ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
    $html = Invoke-WebRequest "https://optifine.net/downloads" -UseBasicParsing -UserAgent $ua

    # Matches OptiFine_<mc>_HD_U_<letter><number>.jar  (e.g. OptiFine_1.20.1_HD_U_I6.jar)
    $matches = [regex]::Matches($html.Content, 'OptiFine_((\d+\.\d+(?:\.\d+)?)_HD_U_[A-Z]\d+)\.jar')
    $byMc = [ordered]@{}
    foreach ($m in $matches) {
        $full = $m.Groups[1].Value      # "1.20.1_HD_U_I6"
        $mc   = $m.Groups[2].Value      # "1.20.1"
        if (-not $byMc.Contains($mc)) { $byMc[$mc] = New-Object System.Collections.Generic.List[string] }
        if (-not $byMc[$mc].Contains($full)) { $byMc[$mc].Add($full) | Out-Null }
    }
    return $byMc
}

function Get-OptiFineDirectUrl($fileName) {
    # optifine.net/adloadx?f=<file> is an ad page that contains the real link.
    $ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
    $page = Invoke-WebRequest "https://optifine.net/adloadx?f=$fileName" -UseBasicParsing -UserAgent $ua
    $m = [regex]::Match($page.Content, 'downloadx\?f=([^"''<>\s]+)')
    if ($m.Success) { return "https://optifine.net/downloadx?f=$($m.Groups[1].Value)" }
    return $null
}

function Run-InstallOptiFine {
    Write-Host "`n--- Install OptiFine ---" -ForegroundColor Cyan
    try {
        # 1. Java for the installer GUI
        $javaExe = Find-Java 8
        if (-not $javaExe) {
            Write-Host "ERROR: Java not found (need Java 8+ to run the OptiFine installer)." -ForegroundColor Red
            Pause-Host; return
        }

        # 2. Fetch available versions
        $byMc = Get-OptiFineVersions
        if ($byMc.Count -eq 0) { Write-Host "No OptiFine versions found." -ForegroundColor Red; Pause-Host; return }

        # 3. Pick a Minecraft version (newest first using Compare-McVersion)
        $mcList = @($byMc.Keys)
        $mcSorted = [System.Collections.Generic.List[string]]::new()
        foreach ($m in $mcList) { $mcSorted.Add($m) | Out-Null }
        $mcSorted.Sort([Comparison[string]]{ param($a, $b) (Compare-McVersion $b $a) })
        $showCount = [Math]::Min(20, $mcSorted.Count)

        Write-Host "`nSelect Minecraft version for OptiFine (newest first):" -ForegroundColor Yellow
        for ($i = 0; $i -lt $showCount; $i++) {
            $mc = $mcSorted[$i]
            $builds = $byMc[$mc]
            $latest = $builds[$builds.Count - 1]   # last listed = newest
            Write-Host ("{0,2}. {1}  ({2})" -f ($i + 1), $mc, $latest)
        }
        Write-Host "01. Enter custom Minecraft version (e.g., 1.20.1)"
        Write-Host " 0. Back"

        $choice = Read-Host "`nEnter number"
        if ($choice -eq '0') { return }
        $mc = $null
        if ($choice -eq '01') {
            $mc = (Read-Host "Enter Minecraft version").Trim()
            if (-not $byMc.Contains($mc)) {
                Write-Host "No OptiFine available for $mc" -ForegroundColor Red
                Pause-Host; return
            }
        } else {
            $idx = 0
            if (-not ([int]::TryParse($choice, [ref]$idx))) { Write-Host "Invalid number!" -ForegroundColor Red; Pause-Host; return }
            $idx -= 1
            if ($idx -lt 0 -or $idx -ge $showCount) { Write-Host "Invalid number!" -ForegroundColor Red; Pause-Host; return }
            $mc = $mcSorted[$idx]
        }

        # 4. Pick a specific build if more than one
        $builds = $byMc[$mc]
        $selectedBuild = $builds[$builds.Count - 1]   # default newest
        if ($builds.Count -gt 1) {
            Write-Host "`nBuilds for ${mc}:" -ForegroundColor Yellow
            # show newest first
            for ($i = $builds.Count - 1; $i -ge 0; $i--) {
                Write-Host ("{0,2}. {1}" -f ($builds.Count - $i), $builds[$i])
            }
            Write-Host " 0. Use newest ($selectedBuild)"
            $bchoice = Read-Host "`nEnter number"
            if ($bchoice -ne '0' -and $bchoice -ne '') {
                $bidx = 0
                if ([int]::TryParse($bchoice, [ref]$bidx)) {
                    # map: displayed index N (newest=1) -> list index
                    $listIdx = $builds.Count - $bidx
                    if ($listIdx -ge 0 -and $listIdx -lt $builds.Count) { $selectedBuild = $builds[$listIdx] }
                }
            }
        }
        $fileName = "OptiFine_${selectedBuild}.jar"
        Write-Host "`nSelected: $fileName" -ForegroundColor Green

        # 5. Ensure parent vanilla version is present (OptiFine inherits from it)
        $parentJson = Join-Path $workDir "versions\$mc\$mc.json"
        if (-not (Test-Path $parentJson)) {
            Write-Host "Parent vanilla version $mc is missing. OptiFine needs it." -ForegroundColor Yellow
            $ans = Read-Host "Download vanilla $mc now? (y/n)"
            if ($ans -eq 'y' -or $ans -eq 'Y') {
                Download-Version $mc
            } else {
                Write-Host "Cannot install OptiFine without vanilla $mc." -ForegroundColor Red
                Pause-Host; return
            }
        }

        # 6. Resolve the real download URL through the ad page
        Write-Host "Resolving download link..." -ForegroundColor Cyan
        $directUrl = Get-OptiFineDirectUrl $fileName
        if (-not $directUrl) {
            Write-Host "Could not resolve download link for $fileName." -ForegroundColor Red
            Write-Host "You can download it manually from https://optifine.net/adloadx?f=$fileName" -ForegroundColor Yellow
            Pause-Host; return
        }

        # 7. Download installer
        $installersDir = Join-Path $workDir "installers"
        New-Item -ItemType Directory -Force -Path $installersDir | Out-Null
        $installerPath = Join-Path $installersDir $fileName
        Write-Host "Downloading OptiFine installer..." -ForegroundColor Cyan
        Invoke-WebRequest $directUrl -OutFile $installerPath -UseBasicParsing -UserAgent "Mozilla/5.0 (Windows NT 10.0; Win64; x64)"
        Write-Host "Saved: $installerPath" -ForegroundColor Gray

        # 8. Open installer GUI (OptiFine installer reads the game dir from the
        #    .minecraft profile by default; launch from workDir so it lands here).
        Write-Host "`nOpening OptiFine installer window..." -ForegroundColor Green
        Write-Host '  -> Press "Install" in the window' -ForegroundColor Yellow
        Write-Host "  -> After it finishes, the OptiFine version appears in the launch menu" -ForegroundColor Yellow

        $startInfo = New-Object System.Diagnostics.ProcessStartInfo
        $startInfo.FileName = $javaExe
        $startInfo.Arguments = "-jar `"$installerPath`""
        $startInfo.WorkingDirectory = $workDir
        $startInfo.UseShellExecute = $false
        $proc = [System.Diagnostics.Process]::Start($startInfo)
        $proc.WaitForExit()

        Write-Host "`nOptiFine installer finished." -ForegroundColor Green
    } catch {
        Write-Host "ERROR: $($_.Exception.Message)" -ForegroundColor Red
    }
    Pause-Host
}

# =========================================================
#  Main loop
# =========================================================
while ($true) {
    Show-MainMenu
    $choice = Read-Host "`nEnter number"
    switch ($choice) {
        "1" { Run-Launch }
        "2" { Run-Install }
        "3" { Run-InstallForge }
        "4" { Run-InstallJava }
        "5" { Run-Settings }
        "6" { Run-DownloadContent }
        "7" { Run-InstallOptiFine }
        "0" { exit 0 }
        default {
            Write-Host "Invalid choice" -ForegroundColor Red
            Start-Sleep -Seconds 1
        }
    }
}
