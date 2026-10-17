# ELYTerminal: wrap the prompt to report folder and title and to mark prompts and commands.
$global:ElyPrompt = $function:prompt
$global:ElyRan = $false
function global:prompt {
    # First: every statement sets $? again.
    $ok = $?
    $esc, $bel = [char]27, [char]7
    if ($global:ElyRan) {
        $code = 1
        if ($ok) { $code = 0 }
        elseif ($Error.Count -eq 0 -or [object]::ReferenceEquals($Error[0], $global:ElyError)) {
            # No new error: a native program failed. Strict mode refuses an unset variable.
            $code = Get-Variable LASTEXITCODE -Scope Global -ValueOnly -ErrorAction Ignore
        }
        [Console]::Write("$esc]133;D;$code$bel")
        $global:ElyRan = $false
    }
    [Console]::Write("$esc]133;A$bel")
    # The user's prompt sees a failed command's $? still.
    if (-not $ok) { Write-Error '' -ErrorAction Ignore }
    $text = & $global:ElyPrompt
    $folder = $executionContext.SessionState.Path.CurrentLocation
    if ($folder.Provider.Name -eq 'FileSystem') {
        $parts = $folder.ProviderPath.Replace('\', '/') -split '/'
        $path = ($parts | ForEach-Object { [Uri]::EscapeDataString($_) }) -join '/'
        if (-not $path.StartsWith('/')) { $path = "/$path" }
        $leaf = (Split-Path -Leaf $folder.ProviderPath) -replace '\p{Cc}', ''
        [Console]::Write("$esc]7;file://$path$bel$esc]2;$leaf$bel")
    }
    "$text$esc]133;B$bel"
}
# A command starts once PSReadLine hands over a line that is not blank.
if (Test-Path Function:\PSConsoleHostReadLine) {
    $global:ElyReadLine = $function:PSConsoleHostReadLine
    function global:PSConsoleHostReadLine {
        $line = & $global:ElyReadLine
        if (-not [string]::IsNullOrWhiteSpace($line)) {
            $global:ElyRan = $true
            $global:ElyError = if ($Error.Count) { $Error[0] }
            [Console]::Write("$([char]27)]133;C$([char]7)")
        }
        $line
    }
}
