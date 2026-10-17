# ELYTerminal: wrap the prompt to report folder and title and to mark prompts and commands.
$global:ElyPrompt = $function:prompt
$global:ElyRan = $false
function global:prompt {
    # First: every statement sets $? again.
    $ok = $?
    $esc, $bel = [char]27, [char]7
    if ($global:ElyRan) {
        $code = 0
        if (-not $ok) {
            # Each native run sets a new $LASTEXITCODE, equal or not; strict mode refuses an unset one.
            $exit = Get-Variable LASTEXITCODE -Scope Global -ValueOnly -ErrorAction Ignore
            $ran = $null -ne $exit -and -not [object]::ReferenceEquals($exit, $global:ElyExit)
            $code = if ($global:ElyNative -and $ran -and $exit -ne 0) { $exit } else { 1 }
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
            $global:ElyExit = Get-Variable LASTEXITCODE -Scope Global -ValueOnly -ErrorAction Ignore
            # A program's exit code is the line's when the line's last pipeline ends in it.
            $global:ElyNative = $false
            $ast = [System.Management.Automation.Language.Parser]::ParseInput($line, [ref]$null, [ref]$null)
            # Strict mode refuses .Count on one statement alone, so the statements stay a list.
            $statements = @(if ($ast.EndBlock) { $ast.EndBlock.Statements })
            $last = if ($statements.Count) { $statements[$statements.Count - 1] }
            if ($last -is [System.Management.Automation.Language.PipelineAst]) {
                $elements = $last.PipelineElements
                $end = $elements[$elements.Count - 1]
                $name = if ($end -is [System.Management.Automation.Language.CommandAst]) { $end.GetCommandName() }
                $info = if ($name) { Get-Command -Name $name -ErrorAction Ignore | Select-Object -First 1 }
                while ($info -and $info.CommandType -eq 'Alias') { $info = $info.ResolvedCommand }
                $global:ElyNative = $null -ne $info -and $info.CommandType -in 'Application', 'ExternalScript'
            }
            [Console]::Write("$([char]27)]133;C$([char]7)")
        }
        $line
    }
}
